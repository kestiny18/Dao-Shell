# v0.1 实现记录

日期：2026-09-16。状态：开发预览，尚待真实模型与干净 Windows 验收。

## 已实现

| 部分 | 当前实现 |
| --- | --- |
| 自然语言入口 | 可配置的 Chat Completions 兼容协议、当前进程多轮对话、工具结果回传、12 次调用上限、按完整轮次裁剪、网络超时与取消 |
| 文件查找 | 限定扫描、名称/扩展名/时间/大小过滤、排序、分页、覆盖范围、会话对象引用 |
| 详情与打开 | 元数据复核、常见文档白名单、目录打开、本地选择与 OS handoff 回执 |
| 文件整理 | 确认、不可变方案、目的目录身份绑定、Windows 源文件句柄移动、祖先目录保护、同卷/不覆盖/20 项上限、创建目录回执 |
| 操作记录 | SQLite 写前记录、逐项回执、重启只观察不重放、Unknown 阻止重复修改、单数据目录实例锁、已完成记录 7 天保留 |
| 资源 | sysinfo 两次采样、CPU/内存/磁盘空间、进程排序；缺失与新进程样本保留未知；不读取命令行和环境 |
| 交付 | Cargo 包、开发与打包脚本、Windows/Ubuntu CI 定义 |

## 实现取舍

1. Everything 尚未接入；按评审决议，限定扫描可以完成第一条闭环。
2. 模型调用“打开”时本地选择具体对象；用户直接使用 `/open N` 或 `search --open N` 时直接提交。没有引入自然语言权限分类器。
3. Windows 目录句柄使用读取目录权限并拒绝共享删除。回归测试证明只读取属性不足以阻止目录改名，已据此修正。移动用 `SetFileInformationByHandle`，禁止覆盖；复核文件身份、大小和修改时间。
4. 资源证据是短时观测，不承诺完整卡顿诊断；新出现或无法观察的进程不会被伪装成有效 CPU 零值。
5. 兼容服务的 API 差异仍需逐个验证。当前没有绑定默认供应商或模型，远程凭据不进入仓库。
6. 未加入应用安装、Noval 依赖、桌面 GUI、插件框架或通用任务系统。

## 验证与待验收

自动化测试包含真实 Windows 临时目录移动、取消、文件替换、同名冲突、重复硬链接、目录保护、记录独占、崩溃窗口核对、Unknown 聚合，以及本地模拟模型的两轮“查找 → 指代 → 确认移动”协议闭环。所有文件修改使用测试夹具，不整理用户实际文件。

真实模型端点尚未配置，设计中的 15 条自然语言固定场景、每条 3 次、总体 ≥90% 的验收尚未进行。“模拟模型”只验证协议、执行层和边界，不能代替模型效果验收。

还需完成：干净 Windows 11 x64 环境解压运行、实际默认应用打开交接、权限不足/跨卷等平台检查，以及真实服务连接与自然语言验收。CI 已定义，但尚未在托管 CI 执行。Linux/macOS 不作为当前完成声明。

本机已通过 21 个自动化测试和 Clippy（警告视为错误），包括目的目录被替换时使确认失效、批次中第二项失败时保留第一项成功及第三项未开始。命令行搜索、500ms 资源采样及 Windows junction 排除也已实跑：目录中的 junction 被跳过，显式把 junction 配为读取根会被拒绝；管道中的 y 不会批准移动。资源采样总耗时包括枚举开销，可能大于指定间隔。可用 `scripts/smoke-windows.ps1` 复现入口检查。

当前 CLI 在等待输入时收到 Ctrl+C，需要 Enter 结束这一行；采样和 HTTP 请求可响应取消。操作结果未知时不会自动补偿或回滚。

## 代码入口

- `src/cli/`：入口与会话流程、参数定义、终端展示与确认分别组织。
- `src/dialogue.rs`：模型协议与有限循环。
- `src/capabilities.rs`：六项能力及严格类型分发。
- `src/files.rs`：范围、查询、对象引用与文档类型限制。
- `src/operations.rs`、`src/storage.rs`：准备、执行、核验与记录。
- `src/platform/windows.rs`：Windows 句柄和打开请求。
- `src/resources.rs`：资源采样。

参考：[工具调用协议](https://developers.openai.com/api/docs/guides/function-calling)、[Windows 文件重命名结构](https://learn.microsoft.com/en-us/windows/win32/api/winbase/ns-winbase-file_rename_info)、[按句柄设置信息](https://learn.microsoft.com/en-us/windows/win32/api/fileapi/nf-fileapi-setfileinformationbyhandle)、[sysinfo](https://docs.rs/sysinfo/latest/sysinfo/)。
