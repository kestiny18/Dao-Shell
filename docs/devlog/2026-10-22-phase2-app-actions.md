# 第二期应用动作：部分原型交付

Date: 2026-10-22
Implementation: IN_PROGRESS（整期未完成）
Launch / explicit force termination: READY（已交付子范围）
Normal close: BLOCKED (`close_identity_not_atomic`)
Self-test: PASS（已开放子范围）
Independent testing: NOT_RUN
Independent review: NOT_RUN

## 版本与范围

基线 `86f0ce6e0cee77a73f2b321b5ae3184ef1df2e8d`；授权计划 `d9ce754`，核心 `4101d62`，最终代码候选 `424ab10`。本记录的检查仅适用于该代码候选及下述 Debug 二进制，不是独立验收或发布结论。

实现四个严格桌面工具及本地入口：`app_targets`、`app_launch`、`app_close`、`process_terminate`。目标发现、不可变计划、本地一次确认、派发前持久回执、普通权限启动、明确实例强制结束、只读核对与重启历史形成闭环。CLI 不开放动作工具。查询共享关闭时模型不能调用，本地入口仍可用；原生路径、身份和确认 token 不进入模型或可恢复工作区。一次请求跨应用动作和 file_open 统一限制，拒绝、取消、不可用关闭后不允许换工具绕过，重复调用不重复执行。

启动显式构造普通用户环境，排除配置 API-key 名称与产品/测试变量；目标 EXE、快捷方式保持核验与 pin，无任意参数、Shell 或 UAC 回退。创建进程不表示应用就绪。强制结束只针对经核验的当前用户具体实例，使用原 handle 观察最多 3 秒，不重复结束；缺失 PID 不当作原实例退出证明。SQLite 最小日志在 OS 调用前记录 dispatching，恢复为 unknown 并保留已有证据，不能自动重放；本地核对无法证明结果时保持未知。

## 必要未完项

专用隐藏窗口试验证实 WM_CLOSE 可被忽略，派发不等于退出；核验后销毁可被复核拒绝。5 秒、2,414 次受控创建销毁未观察到数值重用，不能证明不存在竞态。没有向 stale HWND 发送消息。当前 Windows 接口不能原子校验预期窗口/进程身份并发送关闭请求，因此正常关闭 provider 显式 unavailable，不以一次 IsWindow 或 RuntimeId 声称原子保证，也不自动推荐/替代强杀。

证据 `.tools/phase2-window-boundary-20261022.json`，夹具 `examples/window_close_boundary.rs`。正常关闭是原计划必要能力，故整期保持 IN_PROGRESS，T3 正常关闭 BLOCKED；T0 完成边界判定，T4 仅完成可用子范围验证。

## 自测证据

- 核心全目标回归通过：lib 当时 139 passed / 3 ignored，其他 CLI、文件、配置、索引和协议检查通过。
- 最终动作定向 9 passed / 1 ignored；最终进程身份定向 2 passed / 1 ignored。ignored 原生专用测试另显式运行：`owned_native_launch_environment_receipt_and_force_termination` 1/1，通过普通启动、环境排除/工作目录、创建与核对、原实例结束及短命启动器。
- 桌面 Rust 22/22；最终协议 19/19；最终 UI 70/70。
- 核心与桌面 fmt、Clippy all-targets `-D warnings`、桌面 Debug 构建通过；diff 空白检查通过。
- 最终隔离 WebView c 轮 13/13，20 次本机固定响应请求，无错误：`.tools/phase2-actions-smoke-20261022c/evidence.json`。覆盖不从资源问题自动行动、确认拒绝/停止、重复调用仅一次、子进程环境隔离、关闭不可用阻止替代、专用实例强制结束、历史核对与重启不重放、共享关闭、本地入口与中英键完整性。
- a 轮旧二进制缺少上下文接线，未出现启动预览而失败；修复接线后 b、最终 c 均通过，a 不计 PASS。未重复进行无关全量检查。

截图：`.tools/phase2-actions-smoke-20261022c/launch-confirmation-zh.png`、`.tools/phase2-actions-smoke-20261022c/terminate-confirmation-en.png`。

所有动作只使用专用测试程序、隔离配置与虚构文件；未修改真实开始菜单，未启动或结束办公应用/浏览器。Debug 夹具发现钩子仅在 `cfg(debug_assertions)` 中，默认关闭，要求显式隔离 config、固定 marker/menu 与规范化非 reparse 夹具树；模型无法启用，子进程不继承测试变量。Release 无该钩子。

## 试用

Debug：`E:\WorkSpace\Dao-Shell\desktop\src-tauri\target\debug\dao-shell-desktop.exe`

SHA256：`669B4D1081D8C974F41077F00BB4C70658348B7EFAE8D9241485668E4BAC83E0`

建议重跑隔离夹具，不操作真实应用。输出目录必须尚不存在：

```powershell
.\scripts\dev.ps1 build -CargoArgs @('--locked','--example','phase2_owned_app')
.\scripts\desktop.ps1 build
node scripts/application-actions-smoke.mjs .tools/my-new-phase2-trial desktop/src-tauri/target/debug/dao-shell-desktop.exe target/debug/examples/phase2_owned_app.exe
```

脚本仅创建自己的配置、固定本机响应服务、WebView profile、快捷方式与专用程序副本；正常结束其自己的桌面，强制结束只针对专用夹具实例。无需云凭据；专用程序也会自行超时退出。

未验证：真实云模型理解质量、独立机器/不同权限环境、真实办公应用兼容、实际系统崩溃/磁盘故障全场景、Release 运行与安装包。独立测试和评审未启动。无推送、发布、安装包或服务变更。

本次保留其他工作的三个路径，不纳入提交：`docs/plans/2026-10-17-p15-release-0.1.0.md`、`docs/plans/2026-10-20-v0.1.0-independent-verification.md`、`docs/reviews/2026-10-20-v0.1.0-independent-verification.md`。
