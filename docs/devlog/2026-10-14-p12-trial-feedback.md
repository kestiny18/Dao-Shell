# P12 — 安装试用反馈修复阶段记录

Plan: [P12](../plans/2026-10-14-p12-installed-trial-feedback.md)
Baseline: ca051de6cb9517b6285a4dbeed33206786c0e956 + 已有未提交 P9/P10/P11 与 embedBootstrapper 改动，全部保留。
Independent testing: NOT_RUN
Independent review: NOT_RUN

## T1 — 只读概览（反馈 1/2/3/5）

Implementation: READY（T1 源码与 UI 自测范围；原生布局按主 Agent 安排在 T4 后统一检查）
Self-test: PASS

范围与结果：

- 内存主值按已用 / 总量显示整数百分比，并保留已用、总量、可用容量。缺失、总量为 0 或已用超过总量时显示暂不可用，不夹成虚构 100%；真实零使用量仍显示 0%。
- 顶部 CPU、内存卡片直接展开并滚动至资源详情；新增应用概况卡片，显示登记记录数及已知运行应用数，并展开定位应用详情。已知运行应用不冒充全进程数；采集不可用不伪装成零个应用。
- 卡片使用原生 button、aria-controls / aria-expanded；跳转后焦点到目标 summary，手工收起同步入口状态。尊重减少动画系统偏好；点击只浏览当前快照，不额外采样或请求模型。
- CPU 与内存前列进程改为同时显示的两张表格，使用同一 homeSnapshot，保留采样时间、短时计数窗口、身份变化/消失数量、未知值与覆盖说明。宽度足够时并排，窄屏堆叠，单表横向空间由内部滚动处理。
- 磁盘使用按内容宽度 auto-fit 布局，正常卷卡片可在宽屏排四个一行，窄屏换行；未知可用容量不再因 null 除法误显示全部占用。

修改仅涉及 `desktop/ui/home.js`、`desktop/ui/index.html`、`desktop/ui/style.css`、`desktop/tests/ui.test.mjs` 与本记录。没有修改模型、文件打开、进程结束或服务后端，未构建 / 安装 / prepare payload / 请求 UAC。

自测：

- `node --check desktop/ui/home.js`：PASS。
- 定向 UI 回归 `node --test --test-name-pattern='home remains|overview shows|resource details|memory percentage|overview cards|failed resource' desktop/tests/ui.test.mjs`：6 PASS / 0 FAIL。覆盖同快照双榜、未知值、百分比边界、应用记录与运行语义、展开/焦点/滚动、收起同步及零额外 IPC。
- `npm test --prefix desktop`：48 PASS / 0 FAIL。包含既有会话、权限草稿、应用操作、目录与全盘索引设置回归。

未验证项：按主 Agent 决定，不为 T1 单独构建桌面或安装包；原生宽 / 窄窗口布局在 T4 后统一隔离试用。本阶段没有实机终止进程，也没有把用户 F 盘环境当作当前实验机已复现。T2 / T3 / T4 尚未由本阶段实现，不宣称 P12 完成。

## T2 — 明确确认后结束进程与已确认应用实例（反馈 4/6）

Implementation: READY（T2 实现与隔离自测范围）
Self-test: PASS

实现与边界：

- 新增窄 `process_control` 原生模块，无模型终止工具、任意 PID/路径前端入口、命令拼接或提权。CPU / 内存双次采样读取原生创建 FILETIME，以字符串保存精度；两次创建身份不一致或不可读的榜单行不能绑定结束引用。
- 桌面后端保存 opaque targets（最多 512 项，每组最多 32 个实例，引用 5 分钟），准备阶段重新核验。准备返回 90 秒一次性确认 ID 与实际映像名称、PID、数量、丢失未保存内容提示。等待本地决定期间保留既有全局请求互斥；取消/窗口关闭撤销确认并释放，未知确认不能释放别的请求。UI 超时自动取消。
- 执行只处理准备时冻结的集合，不在确认后重新扩大实例或结束子进程。重开带查询/结束/同步权限的进程句柄，核对高精度创建时间、实际 TokenUser、当前用户会话、完整原生映像及 IsProcessCritical。自身、其他用户/会话、系统 Windows 映像、WebView 辅助进程、索引 worker 与未知身份均拒绝；权限失败不请求管理员授权。
- TerminateProcess 成功只表示发起结束；每个实例限时等待 50ms，分别显示 terminated / already_exited / pending / access_denied / identity_changed / cancelled 等安全结果。集合可能部分完成，不能用“已交给 Windows”冒充已退出。[微软 TerminateProcess 文档](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-terminateprocess)明确其异步语义；[GetProcessTimes](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-getprocesstimes)与[IsProcessCritical](https://learn.microsoft.com/en-us/windows/win32/api/processthreadsapi/nf-processthreadsapi-isprocesscritical)提供本轮使用的原生身份/保护检查。
- 已登记应用补充关联唯一可信开始菜单 Target 的精确 EXE 路径；共享目标、入口变化或同名但不同路径不关联。只记录可核验的当前用户实例，确认列明实际程序 / PID；“已确认实例数”不声称应用所有进程或子进程数。
- 双榜行及已知运行应用卡片提供结束快捷按钮，缺可靠身份/关联时禁用并说明。应用排序继续运行中置顶。结果显示在概览局部区域，确认/执行结束后重新读取双榜和应用；引用、确认及结果不进入模型事实或 workspace 历史。受限权限/保护进程仍可能无法结束，不能把未知状态当未运行。

自测：

- `scripts/dev.ps1 test -CargoArgs @('--locked','--lib','process_control','--','--test-threads=1')`：2 PASS / 1 ignored（自建子进程 helper）。只复制当前测试二进制到独立临时目录并启动受控 helper；真实结束仅该 helper。取消、100ns 创建身份变化、映像变化均保持 helper 存活，最后确认正确身份后结束并回报已退出；未结束任何既有用户进程或服务。
- `scripts/dev.ps1 test -CargoArgs @('--locked','--lib','resources::tests')`：3 PASS，包含原生创建值不进入解释事实。
- `scripts/desktop.ps1 test`：18 PASS。覆盖取消/过期/重放、未知引用、精确创建值、同名不同路径、可信入口变化/共享 EXE 拒绝、冻结多实例不扩集合、pending/partial、全局互斥及取消撤销。
- `npm test --prefix desktop`：50 PASS；新增本地确认、取消不 resolve=true、应用按钮不冒泡运行、部分/pending 局部显示、刷新和不写历史、未知关联禁用、身份失败不派发执行。
- core clippy（`--locked --lib --tests -- -D warnings`）：PASS。
- core / desktop fmt --check、app.js / home.js 语法检查：PASS。
- `scripts/desktop.ps1 clippy -CargoArgs @('--all-targets','--','-D','warnings')`：PASS。

未进行独立 Tester / Reviewer，未构建或安装原生桌面/安装包，未触发 UAC。原生 UI 与最终包试用按主 Agent 安排在 T4 后统一执行。客户另一台含 F 盘电脑的 C/E 更新失败仍 NOT_REPRODUCED，本阶段没有声称修复该问题。

## T3 — 默认全盘名称服务、系统打开与远端安全诊断

Implementation: READY（源码、隔离自测、主 Agent 原生集成与小型安装包检查通过；远端 C/E 故障仍未复现）
Self-test: PASS

- 桌面按本轮已加载 Config.access_mode 选择索引 client，Full 自动使用服务、Restricted 只用已授权目录。旧 enabled=false 侧车原样保留但不再读写或决定路由。设置移除独立启停查询开关；首次明确保存 Full 时，缺少/不匹配/已停止组件提供一次本地安装/更新/启动决定，取消后仍明确兼容目录限制。已有 Full 配置普通启动、状态与搜索只读，不自动安装、升级或重复 UAC。
- 用户点击当前候选走仅本地入口 OpenSelected；模型工具仍使用需要本地确认的 file_open。保留 current last_results、Scope、对象身份、请求互斥、取消与 Windows 句柄复核。不再限制文档扩展名；HTML/EXE/其他类型和目录用固定 ShellExecuteW open，参数与 verb 不来自模型；无关联/系统拒绝只报失败，成功只报 handoff。
- 修复 FileStandardInfo 失败丢失 typed last_os_error。单对象 5/32 继续分类为 Unavailable，不使整个卷因这一分类缺陷失败；87/303、身份变化、USN 响应损坏及未知失败保持严格失败。没有自动重建、Journal 修改、扩大预算或性能调优。
- worker public status 增加可选 diagnostic（安全 stage/code/native_code），成功发布清空，失败保留既有 generation/time。旧 v1 无字段仍可解析。普通状态允许已认证旧版本的固定只读 Status；search/rebuild 的严格 build 核验未放宽。
- 设置可生成/复制净化 JSON：product/protocol/expected+actual build、盘符或匿名卷序号、安全 phase/code/native_code、代次与时间。无 SID/GUID/序列号/私人名称路径/raw异常/凭据。旧服务缺字段明确 unavailable，不猜失败原因。另一台含 F 盘试用电脑的 C/E 持续失败仍 NOT_REPRODUCED；本轮修复明确代码缺陷，不据此声称其根因已确认。

## T4 — 本地过程回执折叠与最终候选

Implementation: READY（源码、隔离自测、主 Agent 原生集成与小型安装包检查通过；远端 C/E 故障仍未复现）
Self-test: PASS

- receipt 使用 details.message.receipt，默认折叠、可键盘展开；覆盖不足/卷失败/打开 handoff 有简短可见摘要。主回答、当前候选直接可用。历史恢复再次折叠，仍不恢复候选或确认。没有把本地回执标成模型内部思考。
- 每次实际 file_search 的 receipt_id 关联事件和最终返回；同次不重复加入回执，不同实际搜索保留各自证据。旧/模拟无 ID 回执用去除临时对象 ID、elapsed 等传输差异的语义键兜底。取消/迟到事件仍服从请求和会话所有权。
- 同步桌面 README/试用指南中的双榜、Full 默认服务、直接打开、结束进程和安全报告说明；保留 CLI 权限与确认语义，不把桌面行为冒充 CLI 自动服务。

本轮检查（未提交工作区 ca051de6 + 全部已有改动）：

- scripts/dev.ps1 test --locked --all-targets：PASS；既有手工 CPU 负载和进程 helper 保持 ignored。包含 mock 模型协议、历史持久化、服务身份/恢复隔离及索引事务保留测试。
- files/runtime open_tests：2 PASS，mock executor 仅合成 HTML/EXE/unknown/无扩展名/目录，直点跳过第二确认，模型拒绝、过期/替换/越界/取消均不派发。
- standard_metadata_preserves_typed_native_error：最终定向 1 PASS，5/32 降为单对象 unavailable，87/303/身份/损坏未知保持失败。
- scripts/desktop.ps1 test：19 PASS；含 Full 忽略旧 preference、Restricted 范围、安全报告字段净化和现有请求/进程确认边界。
- npm test --prefix desktop：54 PASS；含首次 Full 一次组件提示、普通读取不管理服务、折叠/恢复/失败摘要、事件最终去重、不同搜索证据、直点本地 action。
- core 与 desktop all-targets clippy -D warnings、fmt，app.js/settings.js 语法：PASS。

独立两项仍 NOT_RUN；未安装/升级/停止实际系统服务，未请求 UAC，未打开课程文件或终止既有用户进程。已冻结原 95805B 小包，224MB offline 包保留。新 worker/Debug/Release/NSIS 与主 Agent 原生隔离集成证据在随后补充。

### 最终 Debug 与主 Agent 原生隔离集成

- 显式 prepare 新 worker：ACC5A1A6F82EBD489F5EBED3B7240635617DD029178A4ADFAF362F113698FCD4，3,939,328 bytes；未安装。Debug build PASS，冻结于 `.tools/p12-20261014/final-candidate/debug/`，desktop SHA256 `27DF36F738B11A4E4D9A93DD4FCCC6C238623E88310CA30D6A45643A9D3B44AF`，同目录 GNU loader 160,320 bytes 与匹配 manifest。
- 主 Agent 原生 restricted 组 PASS：72% 与容量；宽屏四磁盘一行；窄屏内容无横向溢出（clientWidth/scrollWidth=481）；CPU/内存双榜、资源/应用跳转、已知运行置顶；实际直点仅自建惰性 EXE，无第二应用确认、marker 证明运行；3 个回执默认折叠，当前候选仍可用；隔离配置未修改，自己的窗口已关闭。
- 主 Agent 原生 Full 组 PASS：legacy enabled=false 原样保留但 status.enabled=true；无独立启停查询控件；报告实际04B/预期ACC5且无 SID/GUID/私人路径；严格 version_mismatch 可见，无安装/升级/提权请求；隔离配置未改，自己的窗口已关闭。
- 证据 `.tools/p12-20261014/native-trial/native-evidence.json` 与 `.tools/p12-20261014/native-full-trial/native-evidence.json`；宽屏截图目视正常。窄屏首次 compositor 截取偏移，补图过程及派生脚本纠正见下段。本记录为实现方/主 Agent 集成自测，独立两项仍 NOT_RUN。

原生布局补图纠正：主 Agent 首次派生 `.tools/p12-20261014/native-layout-trial` 的脚本因 CRLF split 未切断业务段，重复运行了自建惰性 EXE，随后因新目录 marker 不匹配失败。该次仅自建夹具，不涉及用户数据或服务，已标 INVALID_HARNESS，不能作为产品验收失败或通过证据；保留 harness-correction.json。随后 v2 明确只运行布局，fromSurface=false/scroll0，747px 窄屏内容宽481且无溢出，宽/窄截图目视 PASS。最终有效布局证据为 `.tools/p12-20261014/native-layout-v2/native-evidence.json`、`narrow-layout.png`、`wide-layout.png`，无需产品修改或重做业务测试。

### 最终 Release / NSIS 安装包

- scripts/package-desktop-windows.ps1 -Jobs 2：PASS，Release 与 embedBootstrapper NSIS 构建成功；打包未执行安装包或系统服务管理。
- 新安装包 `dist/Dao-Shell-0.1.0-windows-x64-setup.exe`：9,072,494 bytes（约9.07 MB / 8.65 MiB）。SHA256 `D6F32284222E7B5CFC34ABD9DDB6DCE7D9437AAC7BA4210374D2476B026F5076`，旁边 .sha256 与实际字节一致。
- Release desktop：35,752,960 bytes，SHA256 `4B4BDB731CF0703FAF72C126B31846C3BC1F70D5D8DBA7456A11A9AED93CFB54`；匹配嵌入的 ACC5 worker 字节与 manifest 摘要，x64 Windows GUI subsystem 检查 PASS。
- 新目录 `.tools/p12-20261014/final-candidate/verify-package.py` 检查 PASS：NSIS embedBootstrapper、1,860,304-byte bootstrapper、既有运行环境检测与失败中止分支、GNU loader 160,320 bytes、最新中文指南、完整 worker 字节/摘要、x64 GUI、安装包 SHA 与冻结副本。
- 旧 9,026,733-byte 引导小包 95805B 已冻结 previous-bootstrapper-setup.exe；224,145,432-byte B18CC 离线包原样保留于 dist。没有覆盖 `.tools/webview-bootstrapper-20261014` 历史证据。
- 最终冻结安装包、Release desktop/DLL、manifest、installer.nsi、checksum 与 artifacts.json 均在上述新 final-candidate 目录；Debug 与原生证据版本已在前段明确记录。同一产品源码，没有 native 结果后产品修改。

P12 本轮实现/自测/原型产物可交接；主 Agent 决定 Plan 最终状态。Independent testing: NOT_RUN；Independent review: NOT_RUN。另一台试用电脑 C/E 失败 NOT_REPRODUCED，仍需其安全报告与必要显式组件更新后的证据；构建不代表已安装新版服务。干净机器、缺少 WebView2 的联网引导安装、离线失败实装以及 P11 升级/回滚/重启等真实生命周期仍 NOT_VERIFIED；未重试已取消 UAC，未提交/推送/合并/发布/迁移历史。
