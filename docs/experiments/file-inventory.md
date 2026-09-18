# File inventory experiment

这是 Windows 文件索引的第一步实验，不是全机搜索或后台 Runtime 的交付。普通用户运行一个明确目录的快照基线，并单独测量 USN 日志访问能力。实验不会调用模型，不读取文件内容，不移动或打开源文件，不修改 Dao-Shell 的配置或写权限。

## 运行

在仓库根目录的 PowerShell 中：

```powershell
.\scripts\dev.ps1 build -CargoArgs @('--locked', '--example', 'inventory_spike')
.\target\debug\examples\inventory_spike.exe --help
.\scripts\inventory-smoke.ps1 -FileCount 20000
.\scripts\inventory-watch-smoke.ps1
```

烟雾测试仅创建自己的测试目录，结果和样本保留在 `.tools/inventory-smoke-<随机标识>/`。`report.json` 记录 SQL 查询耗时与覆盖情况，不会上传。依次验证独立进程读取、目录改名、文件删除、排除目录联接。测试样本不包含真实用户文件。

手动检查一个明确目录（数据库父目录需预先存在，且数据库必须位于扫描目录之外）：

```powershell
.\target\debug\examples\inventory_spike.exe probe --root 'E:\WorkSpace\Dao-Shell\src'
.\target\debug\examples\inventory_spike.exe scan --root 'E:\WorkSpace\Dao-Shell\src' --db '.\local\inventory.db'
.\target\debug\examples\inventory_spike.exe query --db '.\local\inventory.db' platform windows
.\target\debug\examples\inventory_spike.exe status --db '.\local\inventory.db'
.\target\debug\examples\inventory_spike.exe watch --root 'E:\WorkSpace\Dao-Shell\src' --db '.\local\inventory.db' --seconds 60 --reconcile-seconds 30
```

`local` 不存在时可先执行 `New-Item -ItemType Directory -Path .\local -Force`。子命令输出 JSON；`watch` 先逐行输出快照事件，退出时输出汇总。需要 Ctrl+C 时，在下一次检查取消状态时停止；单次文件系统调用本身没有硬超时。默认扫描预算为 100,000 个访问条目、60 秒，包含目录与被跳过的条目。监听时长从首次扫描后起算，不是进程的硬超时。

## 已实现的行为

- 一个数据库绑定一个目录；相对路径的多个关键词按 AND、大小写折叠后的字面子串匹配，`%`、`_` 不作为 SQL 通配符。排序仅按路径，不宣称与正式搜索排序相同。
- 使用现有平台文件身份读取，保存文件名、路径、身份与大小/修改时间，不读内容。路径文本保存在本地数据库中；尚无数据库加密或多用户服务。
- `scan` 在事务中建立新一代，成功后一起提交记录与快照说明。预算耗尽、取消和数据库错误回滚，保留上一代。
- 遍历/属性读取错误计数并保留最多五条样本；完成时如有错误明确标记 `partial_scan_with_errors`。这是部分观察，不是文件系统的原子快照，不能据此断言漏项已经删除。
- 符号链接、Windows 重解析点不被跟随；重解析点本身也不进入结果。不可表示为 Unicode 的路径不收录并计入错误。特殊文件也计入遗漏。
- 查询读取同一事务内的快照说明和条目；每次携带 `manual_snapshot_not_live` 或 `event_assisted_snapshot_not_live`、扫描起止时间、覆盖范围，以及可选的最后增量更新时间。磁盘离线、文件变化后仍可能返回旧记录，不能直接拿这些记录执行操作。
- 数据库需在扫描树之外；拒绝修改非本实验的 SQLite 数据库。数据库文件与父路径不接受链接。这个实验不是对抗并发恶意路径替换的安全索引服务。
- Windows `probe` 查询文件系统和 `FSCTL_QUERY_USN_JOURNAL`。不会申请提权、创建或删除日志；分别报告可用、拒绝访问、日志未启用等状态。成功也只代表日志元数据可查询，不证明事件读取和恢复已经可用。

## 普通权限通知实验

`watch` 使用实验专用开发依赖 `notify 8.2.0`，Windows 后端为 `ReadDirectoryChangesW`；不进入正式 Shell 的依赖执行路径，不提权。先挂监听，再初次重扫，以便处理建库期间的变化。

- 每约 200 ms 合并一批通知，对明确的普通文件重新读取当前身份/元数据并增量写入；删除事件也依据当前存在性判断，不盲目应用事件顺序。
- 目录变化、链接/权限等无法安全判断的对象、部分覆盖的旧快照，退回完整重扫。部分增量若遇到回退条件，整个事务撤回，不能留下半批更新。
- 原生回调使用容量 1,024 的非阻塞队列；本地队列溢出记录丢失标记，触发重扫。显式 Rescan 标记与监听错误同样触发重扫。
- 默认每 30 秒额外重扫（可设 1–300 秒），用于修复没有显式丢失信号的情况。已核查锁定的 notify 8.2.0 Windows 源码：不能假定所有原生缓冲区溢出/后端终止都能回调为 Rescan/Error，因此这个兜底不可省略。当前不会自动重建故障通知订阅，后台失效时靠周期扫描兜底。
- 重启总是完整重扫，没有伪造可恢复游标。监听根目录身份改变则报错停止；数据库不会留下“正在监听/实时完整”的持久标记。

`inventory-watch-smoke.ps1` 在隔离样本上检查原生新增、修改、删除、父目录改名，强制结束实验进程、停机期间修改后的重启恢复，以及无事件时的周期重扫。它只结束自己启动的隐藏实验进程。单元测试注入本地队列溢出与 Rescan/Error；没有强制制造原生 Windows 缓冲区溢出。

## 实验没有证明的事情

- 没有 MFT 枚举、USN 事件消费、持久游标、USN 日志缺口检测/恢复或开机运行。普通通知实验在显式运行期间自动更新，退出后不继续工作。
- SQLite `instr` 是元数据遍历基线；大量命中的第一页可能提前停止，不能代替稀有匹配和零结果查询的测量。没有百万文件规模的性能承诺。
- `query_ms` 只包含 SQL 执行和结果解码，不包含打开数据库、进程启动、JSON 输出、模型延迟。独立进程不等于冷磁盘缓存。
- 测试不代表受保护目录、在线云文件、网络卷、所有文件系统或 Linux/macOS 已经获得产品支持。
- 当前入口不接入 LLM 和正式 `file_search`，没有“全机写入模式”。

## 下一项实验的验收条件

1. 单独验证 NTFS 索引路径需要的权限，普通用户默认路径必须继续可用；如需特权，明确最小辅助进程边界，不能提升整个 LLM/UI。
2. 在隔离测试卷/样本中测试增量读取、父目录改名、日志 ID 改变、游标落后、重启和中断。索引数据与消费进度需要一起提交，缺口要显式进入重扫状态。
3. 增大样本并分别记录首次建库、稀有/零结果查询、内存、数据库大小、更新延迟，再决定查询结构和常驻方式。

随后做最小 Tauri 入口，用真实的第二个调用方提炼接口。桌面第一条链路为自然语言输入 → 选择候选 → 打开请求及结果；移动需要专门设计绑定方案的确认协议后再接入。会话状态与共享索引分开，先不启动 HTTP 服务或 SDK 工程。

## 原生接口依据

- [FSCTL_QUERY_USN_JOURNAL](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-fsctl_query_usn_journal)
- [GetVolumePathNameW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getvolumepathnamew)
- [GetVolumeInformationW](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-getvolumeinformationw)
- [notify 8.2.0](https://docs.rs/notify/8.2.0/notify/)
