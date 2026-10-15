# P11 — 实机服务验证检查点

Implementation: IN_PROGRESS。Self-test: 普通客户端认证与实际 USN 夹具功能 PASS；生命周期/恢复待补；性能 FAIL（用户已延期优化）。Independent testing: NOT_RUN。Independent review: NOT_RUN。

2026-10-14 用户在获知 P11 尚缺实机验证、此前 UAC 取消之后明确要求“那请继续执行吧”。主 Agent 承接该指令重试一次固定安装，未从自动任务推导管理员授权。没有提交、推送、合并、发布或迁移历史数据，不自动重启/休眠用户电脑。

## 固定候选与隔离配置

- Release desktop：`E00F6D9DE387BA1DB86D7FEC552B2CE8C9A26C02810FF86597EB8497E759C1A6`。
- Worker：`04B9986FD2A0F625D88EE9A94C5CE8FF6AA4240C86D663EA3534D46C910316DB`。
- `.tools/p11-20261014/live-service-trial/`：独立 restricted、无模型配置；操作 roots 仅 `allowed/` 虚构文件，不改变用户配置。启用后真实全盘仅采集名称及必要元数据，来源文件操作只使用该夹具。
- 通过真实 Release 桌面的固定 `enable` 入口触发安装，验证固定 payload/SHA、提权前源文件与祖先句柄保护链；没有直接以临时任意 EXE 改写 SCM。

## 新实机证据

09:28:24 安装返回 completed / exit_code 0。SCM 查询：固定服务 `DaoShellFileIndex` 已安装，LocalSystem、automatic（start_type=2）、Running（state=4）、PID 27452，ImagePath 指向 ProgramFiles 下固定 SHA 目录。受保护 manifest current 与 worker SHA 相同，owner 为实际安装用户。普通用户对 ProgramData 服务数据目录 READ_CONTROL/attributes 打开返回 ERROR_ACCESS_DENIED，ProgramFiles/manifest/父目录可读权限核查成功。安装后隔离操作配置字节未变。

**普通桌面连接 FAIL**：状态 unavailable；CLI service-health 报 `Win32 handle: 拒绝访问 (os error 5)`。只读 native 诊断逐项排查，确认 `OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, PID27452)` 返回 5，不能把服务进程映像可查询作为普通客户端的必要条件。

同一普通客户端实际管道诊断：带 READ_CONTROL 的 CreateFile 成功；内核 server PID=27452、server session=0；`GetSecurityInfo(SE_KERNEL_OBJECT)` 读取 owner 成功，为 BUILTIN Administrators（S-1-5-32-544）。这只证明该 API 路径，不等于认证修复或查询通过。证据 `.tools/p11-20261014/live-service-trial/{install-evidence,access-diagnostic}.json`。服务保留运行以供新普通客户端复验，未重装、未改其进程/token ACL。

## 当前修复

主 Agent 显式恢复同一 Coder：保留 SCM fixed own-process/LocalSystem/Running/PID/ImagePath、受保护 manifest/image SHA；以连接句柄的内核 peer PID/session、受保护 owner 和严格 DACL 绑定服务，前后复核 SCM/peer，去除普通客户端必须 OpenProcess SYSTEM 的依赖。不采用响应自报身份，不放宽 SYSTEM 进程或 token 权限。先以新普通客户端对现存固定服务复验，管理员升级由主 Agent 后续协调。

依据：[Named Pipe Security](https://learn.microsoft.com/en-us/windows/win32/ipc/named-pipe-security-and-access-rights)、[GetSecurityInfo](https://learn.microsoft.com/en-us/windows/win32/api/aclapi/nf-aclapi-getsecurityinfo)、[GetNamedPipeServerSessionId](https://learn.microsoft.com/en-us/windows/win32/api/winbase/nf-winbase-getnamedpipeserversessionid)、[SE_OBJECT_TYPE](https://learn.microsoft.com/en-us/windows/win32/api/accctrl/ne-accctrl-se_object_type)。文档描述了 SD 读取/READ_CONTROL 和内核对象 API；其安全组合是本实现的设计选择，仍需实现自测与实机复验。

## 修复后原生功能证据

新普通 CLI `2CFE14924D1180E85C5771A5EE568983A5036C032BB1F1B2CE392657A5C8442B`，Debug 桌面 `91C2A3D6904778F00271DA6CB5FC7F6BE83C806EFEA0C6A94070301A7E6E54A0` 与 Release 桌面 `4BBCEBDFAD3DAF6FE060A77919EE5400EA22FB4DFF25BEB0130192CB0602AECE` 均对原服务 `04B998…` 通过普通权限认证；未重装服务或弱化 SYSTEM 进程/token ACL。该混合版本仅用于定位与复验，最终统一候选仍待构建。

Release 原生桌面无模型查询虚构关键词：source=windows_service、返回1条且显示在界面，restricted 操作配置字节未变。C/E 已发布，仍准确显示 coverage_incomplete；D ready。未知覆盖未冒充完整。

真实 SYSTEM USN + 普通 Release WebView 的45步夹具生命周期 PASS：3个硬链接指向同一完整原生身份；中文父目录改名后全部路径更新；逐一删除链接至0，其他名称不误删；20轮创建/删除正确。操作范围之外的虚构候选能被发现，但打开被 Scope 拒绝，没有扩大操作权限。Debug 亦完成对应功能验证。证据 `.tools/p11-20261014/live-service-trial/{native-fixed-auth,usn-live-evidence,usn-live-evidence-release}.json`；这里只证明夹具功能，不代表所有真实文件完整覆盖。

## 性能基线及用户延期决定

Release 60次实际桌面查询（常见/罕见/无命中各20，首屏最多50）：UI p95 1110.5 /1094 /1029.3 ms，后台 p95 1040 /989 /967 ms。20轮创建首次可查 p95 4158.6 ms。两项均未达原目标。热 OS 缓存、未刷缓存，服务实际维护卷；查询仅使用虚构文件名。功能测试中的 PASS 不代表时延目标 PASS。证据 `query-release-baseline.json` 与 `usn-live-evidence-release.json`。

进一步只读拆分10轮：health认证 p95 14.46ms，C/E/D 无命中查询 p95 677.16 /247.17 /38.33ms；名称 SQL 是主要查询成本，未据此实施缓存。证据 `client-profile-samples.jsonl`。540543条合成核心单文件更新约492–514ms，仅作诊断，不能替代 USN/IPC/UI。

2026-10-14 用户明确“查询时间上的优化先不做，优先功能完整性”。已停止拟议名称内存快照/查询加速及性能实验，原目标和 FAIL 作为后续优化依据保留；不把这些时延作为本次功能原型就绪门槛。

## 当前继续范围与保留缺口

继续修复卷原因展示与明确功能缺陷，验证桌面退出后的维护、服务重启持久续读、固定生命周期入口，最后构建统一 worker/desktop 候选。实际升级/回滚/修复/卸载、开机/休眠/物理卷恢复仍需相应证据，未验证不作 PASS；不自动重启或休眠宿主机，不删除真实 Journal 或历史索引。安装 exit0 不代表完整首建。P11 保持 IN_PROGRESS，定时任务按最新功能优先范围继续。

## 功能优先续作：桌面退出与重新打开

`4BBCEB…` 普通 Release 桌面实际退出（核对自身 PID 消失）后，`04B998…` SYSTEM 服务维持原 PID27452。只在 `.tools/p11-20261014/live-service-trial/allowed/` 创建新的虚构文件，普通 CLI 在桌面关闭期间已查到该文件；再打开同一隔离桌面，无模型输入返回 source=windows_service、1条可见候选，操作配置字节未变。此功能检查 PASS，证据 `functional-background-v2.json`。

第一次 harness 未处理 CLI 非零返回而停止，保留 `functional-background.json` FAIL；随后只读命令已查到首个夹具。改进 harness 将结构化暂时失败保留并轮询，用第二个独立名称重测 PASS（本轮无暂时失败）。没有将第一次失败日志覆盖成成功，也没有修改生产查询算法。

已通过本地明确管理请求触发一次 `service-stop` UAC，返回 `uac_cancelled`，实机续读检查未执行。SCM仍为原 PID27452 Running；没有触发 service-start、没有执行停机夹具改动，也不重复请求或绕过取消。不自动重启电脑。证据 `functional-service-restart.json`（FAIL为检查未完成，取消已被正常识别）；此后的服务重启/升级/修复/卸载实机工作等待用户后续明确指令，先完成普通权限实现/构建。

## F1 新桌面普通权限集成自测

固定 Debug `2C78C8CB15309D65F65149D8E2A71AE720DCCB6DD24430EE250CB4D516E41210` 继续配原 `04B998…` 服务，实际 native 检查 PASS（`f1-native-functional.json`）：

- 认证现存服务，设置页显示 running；新状态渲染函数正确显示 maintenance_stopped 和代次42，未知虚构私有码不透传。真实服务端未部署 F1，因此其不支持卷状态修复仍以隔离测试为证据。
- 本地确认停用只保存独立偏好；同一虚构名称使用 source=scan 的兼容目录搜索返回1条。
- 重新启用已在运行的服务没有调用提权管理；相同虚构名称恢复 source=windows_service、1条。
- SCM原 PID27452未变，Scope配置字节未变；自己的原生窗口已关闭。

此次是主 Agent 实施集成自测；独立测试/评审仍 NOT_RUN。统一候选将重新嵌入含F1修复的worker，之后在原服务上出现版本不匹配是预期，需要后续明确升级；不能把旧payload诊断客户端冒充统一服务升级完成。

## 最新统一候选与交付限制

正式打包及固定字节校验 PASS：worker `F5882D1CA4195DBD511EC9B057F0EE38B771A83841E7D0947A8340D9D46668B1`，Release桌面 `43FC94E422227F14BBF395D06993D4F4F0E2BEA807F6B92491CBDD538ECE695B`，NSIS `B18CC65D14B0BB599A7C9432DA54BB0BCAD29D0F3A21391732E9FBA71FAB84AC`。固定副本与旧A9包保留在 `.tools/p11-20261014/function-unified-candidate/`，含payload/SHA/x64 GUI/副本校验结果，详细源码与自测见 [本轮记录](2026-10-14-p11-function-completeness.md)。未运行安装包。

主Agent对新统一Release普通原生检查PASS（`unified-native-version-mismatch.json`）：明确版本不符、指向手动更新、不展示无效重建、enable本地拒绝；在原Scope目录的兼容搜索返回1个虚构结果，source=scan；没有扩大权限或改动服务，自己的窗口已关闭。实际运行的服务仍是04B、PID27452，本轮服务端状态修复尚未部署。剩余实际升级/修复/回滚/卸载、服务重启续读与主机/物理卷恢复保持NOT_VERIFIED；取消提权不重试，先交付当前功能源码与构建证据。P11整体IN_PROGRESS，独立验证NOT_RUN，定时续作保留但相同阻塞静默。
