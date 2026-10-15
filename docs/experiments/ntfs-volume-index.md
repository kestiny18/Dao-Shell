# NTFS 全卷 MFT / USN 专用实验

对应 [P10](../plans/2026-10-02-p10-volume-index-exploration.md)。与桌面 P9 后端隔离，不替换生产搜索。

## 程序与命令

```powershell
.\scripts\dev.ps1 build -CargoArgs @('--locked', '--example', 'ntfs_inventory_spike')
.\target\debug\examples\ntfs_inventory_spike.exe probe --volume E:
```

程序不自行提权。需要提升时，只提升已固定并校验的专用可执行文件，由正常 Windows UAC 交互决定；不提升模型、桌面或任意脚本执行器。程序只请求卷 `GENERIC_READ`，没有修改/创建/删除 Journal 的接口，没有安装服务或计划任务。

输出路径使用新建文件，`--report` 拒绝覆盖和已存在的链接父目录。实验数据库须为绝对路径，父目录事先建立，放在 `.tools/p10`。数据库和任何真实路径记录不入版本库、不上传；命令仅输出计数和性能，不打印查询结果路径。

```powershell
# 以下由主 Agent 协调受限专用程序的权限，不直接当作普通权限成功示例。
ntfs_inventory_spike.exe build --volume E: --db E:\WorkSpace\Dao-Shell\.tools\p10\e-index.db --report E:\WorkSpace\Dao-Shell\.tools\p10\build.json
ntfs_inventory_spike.exe replay --db E:\WorkSpace\Dao-Shell\.tools\p10\e-index.db --report E:\WorkSpace\Dao-Shell\.tools\p10\replay-01.json
ntfs_inventory_spike.exe query --db E:\WorkSpace\Dao-Shell\.tools\p10\e-index.db --query p10fixture
ntfs_inventory_spike.exe status --db E:\WorkSpace\Dao-Shell\.tools\p10\e-index.db
```

## 首次建库与增量机制

- 核对 NTFS、卷序列号和卷 GUID，以 GUID 打开卷。普通权限 probe 分别尝试 access=0 与 GENERIC_READ，分别记录 QUERY、ENUM 第一页和 READ 调用；probe 不代表枚举完成。
- 建库先记录 Journal ID 和 NextUsn，再 `FSCTL_ENUM_USN_DATA` 枚举直到 `ERROR_HANDLE_EOF`。没有 P9 的条目/时间/目录深度预算，不调用目录递归，也不逐个文件 stat。
- 记录 FRN、父 FRN、名称、USN 和属性。名称不含正文、大小或修改时间；不为了补齐字段逐个打开文件。
- 枚举期间修改以保存的 NextUsn 为起点回放；同一 FRN 的更旧记录不覆盖更新的 MFT 记录。回放到开始回放时捕获的水位后重新核对日志状态，再原子提交索引和游标。
- 后续 `replay` 从数据库持久游标读取 USN，无需 MFT 全扫；整个回放及游标在同一事务。Journal ID 改变、有效范围缺口、卷变化、未知记录版本、读错误等使本次事务回滚并返回错误。
- 父目录更名更新父 FRN 图中的一个节点，查询时解析路径；不批量重写子路径。断链和循环记为未解析，不能算作完整路径覆盖。
- 报告 `enum_ms` 包含枚举与 SQLite 写入，`ready_ms` 还包含追赶回放、提交及状态统计。`process_memory_bytes_after_build` 是结束时的瞬时值，不是峰值。查询计时含数据库打开、匹配及命中路径解析，不含进程启动。

## 明确限制

- 每个 MFT 记录保留一个名称，不保证硬链接所有名称。出现 USN HARD_LINK_CHANGE 时拒绝继续提交，要求重新建库/后续补充名称协调；不能冒充所有路径已经覆盖。
- 仅支持 USN V2.0 记录；其他版本、损坏长度和非 Unicode 名称会失败，不静默跳过。
- 查询目前是文件名文字包含的 SQL 扫描，返回命中数和可解析数量；不代表 Everything 的完整查询语法或内存检索性能。广泛命中会逐个解析父链，本轮先验证卷级采集与游标正确性。
- 卷上系统元数据和重解析点会计数，但不沿重解析点进入其他卷。离线卷、非 NTFS、全部硬链接和多卷汇总未实现。
- 数据库存储原始名称，不加密。父路径链接检查是实验防误用措施，并非可抵抗恶意并发路径替换的提权服务安全边界；不应作为面向不可信调用者的长期服务部署。
- 退出后不监听，下一次显式 replay 补读；没有自动启动或常驻服务。全卷建库如果失败，已提交旧代保留，新数据库可能只有空 schema，不能查询为完成状态。

## 实现级验证

2026-10-02，基线 `ca051de` + 保留的 P9 工作区，仅新增本实验三个源码文件和本说明，无产品路径调整、无依赖修改。

```powershell
.\scripts\dev.ps1 test -CargoArgs @('--locked', '--example', 'ntfs_inventory_spike')
.\scripts\dev.ps1 clippy -CargoArgs @('--locked', '--example', 'ntfs_inventory_spike', '--', '-D', 'warnings')
```

9 项合成/隔离测试通过：非法和未知 USN 记录、日志 ID/游标范围/卷序列号变化、目录改名和删除、较旧回放不覆盖新快照、硬链接异常回滚、SQLite 错误回滚索引和游标、孤儿/循环图、拒绝无关数据库、报告拒绝覆盖。Clippy 无警告通过。

普通 token：access=0 可打开卷，但 QUERY 返回 Win32 1；GENERIC_READ 在打开卷阶段返回 Win32 5。`build` 同样在打开阶段失败，不创建数据库。此结果不证明提升后的接口不可用。

真实提权枚举规模、两轮真实夹具变化与退出后续读、性能数据已经核对，见 [P10 实测表](../plans/2026-10-02-p10-volume-index-exploration.md#actual-volume-evidence--2026-10-02-run-recovered-2026-10-10)。这些结论只适用于所列 E: 卷、固定产物和测试夹具，不代表常驻监听或全部路径覆盖。Independent testing: NOT_RUN；Independent review: NOT_RUN。

## 2026-10-10 缺失父链诊断

只读检查上次持久化数据库：540796 条记录，游标 585236088。8 条未解析记录无循环；其中 7 条最终指向缺失父 FRN `000b00000000000b`（MFT 槽位 11），另 1 条指向 `000100000000001d`（槽位 29）。数据库内不存在对应父记录，也没有相同槽位不同序列号记录。

当前只保存 ENUM/READ 返回记录，并仅对根节点槽位 5 作特殊处理，没有缺失父节点补全。前 7 条符合系统元数据父链缺失特征；不能称为 7 个普通用户目录丢失。槽位 29 的身份和新增未解析条目的事件来源尚未证实，不能按猜测过滤或认为删除已完成。此次未输出实际名称/路径，未修改数据库或产品代码。

## 产品路线结论与剩余验证

E: 实测支持继续采用 MFT 首次建库、持久 USN 游标续读路线；普通目录遍历保留为非 NTFS 等场景的兼容候选。Microsoft 的 [ENUM 接口说明](https://learn.microsoft.com/en-us/windows/win32/api/winioctl/ni-winioctl-fsctl_enum_usn_data)明确该方式适用于 NTFS，不能据此声称网络盘或其他文件系统同样支持。

下一步应处理缺失父链、硬链接多名称、未知记录版本及日志失效重建，再测多卷覆盖、长期更新和更大命中量搜索。SQL 全表包含查询已有约 200 ms 的基线，后续需单独优化查询结构；不能把 USN 回放的几毫秒当作搜索延迟。

建议后续产品使用职责受限的索引辅助进程，UI/模型维持普通权限。索引数据库由辅助进程管理；限制可接受请求和输出，不提供任意命令/文件写入接口。文件名可见范围需与用户权限明确对应，实际文件读取/修改仍经过原有权限及确认边界。本轮没有安装服务、删除目录配置或把索引权限授予模型。
