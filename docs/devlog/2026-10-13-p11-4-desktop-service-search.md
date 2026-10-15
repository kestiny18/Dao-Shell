# P11.4 桌面全盘名称搜索接入（隔离原型候选）

Implementation: READY（仅本 Task 隔离原型与自测范围；P11 整体验收未完成）。Independent testing: NOT_RUN。Independent review: NOT_RUN。

2026-10-13 19:45 heartbeat 后恢复同一 Coder。保留 P9/P10/P11 已有工作；P11.3 冻结产物 `2D38C50973D9212808AAE28E2673D87F4BF2E13CDEB7082D8B59BC08C32AA01B` 副本由主 Agent 保存，未改。无 UAC、真实服务安装、提交推送或下级 Agent。

## 已确认最小范围

基本 literal query + 默认 Relevance + 第一页走全盘服务，在 Scope.read 默认根目录约束之前执行；服务仅按系统调用者过滤名称。Objects 仍每会话独立，候选必须重新核 GUID→DOS 映射、serial、FRN，真正 inspect/open/write 不放宽原 Scope/身份/确认。复杂 terms/path/metadata/其他排序/后续页保留全部条件，仅在列出的兼容目录中有界补充；无兼容目录给专门解释，不要求为全盘名称发现登记目录。模型优先 query 不能牺牲用户约束。

## 当前检查点

- `src/search_index.rs`、`src/search_index/windows.rs`：小型 IndexClient、跨卷 receipts、固定 SCM 服务客户端，单次 3 秒总预算及用户取消桥接；不接受客户端 DB/EXE 路径。每卷首屏最多 50 项，返回动态覆盖/代次/更新时间及静态原因标签。
- `files.rs/session.rs/dialogue.rs`：候选发现独立于操作 Scope；新引用重新核身份，checked 保持原权限；reset 保留后端连接但清空对象；模型提示明确 query 与 terms 的不同覆盖。
- 映射严格拒绝绝对 suffix、..、冒号/设备路径；候选创建前后重新核挂载 GUID 与 serial。同序列号但不同 GUID 不能生成引用。
- `search_index/install.rs`：仅显式本地设置操作调用固定 RunAs；bytes/SHA 固定，提权前重新核源文件并持有源与祖先句柄，搜索/状态不调用。真实启动未执行。
- `desktop/src-tauri/src/index_service.rs`、bridge/main：设置独立启用标记，固定管理动作；搜索线程与管理操作分离。legacy P9 保留。
- `scripts/prepare-index-payload.ps1`：显式构建并生成 worker bytes + SHA manifest，build.rs 只核验/嵌入，不递归构建、不开 UAC。缺产物时 UI 明确不可安装，包装脚本要求预备固定产物。
- 设置新增状态、覆盖与显式服务管理；普通结果显示索引 receipts；全部文本用 textContent。生产服务 report 入口继续关闭。

目前 `scripts/dev.ps1 check --locked --lib` PASS；`scripts/desktop.ps1 check` PASS（最近 6.21s）。桌面 lock 仅接入已有 sha2/serde_json 为 build dependency、tempfile 运行依赖；离线更新报告 0 个新增版本。已 fmt，但实现级边界测试、完整 clippy、最终 worker/payload/桌面构建和 native 验证尚未完成，当前不得称 READY。

## 下一步（本 Task 内）

补 mock client/session 权限、条件保留/取消/失败补充回归；映射恶意 suffix 与同 serial 不同 GUID；设置管理无确认/无 payload 不提权、UI 不隐式安装/回执；运行受影响 Rust/UI 测试和 clippy，显式准备 payload 并构建桌面给主 Agent 的隔离 native 夹具。真实 SYSTEM/UAC/全盘性能与卷恢复仍未验证，不在这里伪报通过或擅自开始 P11.5。

## 最终候选与自测（2026-10-13）

上述“当前检查点/下一步”是中途恢复记录；本节为本 Task 最终交接证据。源码仍在基线 ca051de 加 P9/P10/P11 未提交工作区上，未提交推送。最终候选：

- Desktop Debug：`E:\WorkSpace\Dao-Shell\desktop\src-tauri\target\debug\dao-shell-desktop.exe`，SHA256 `7E1E45C33003B49005B91F0B6A33556F4C72B8816B2FD9591CD395E660252F2D`。
- 嵌入 worker / `target/release/dao-index-worker.exe`：SHA256 `A3891C2FE880E415032D393B2E5397275730C5216875AE1DA1D84C1E77BD20D7`。自动生成 manifest 与 bytes 存在被忽略的 `desktop/src-tauri/index-payload/`；未来源码变化需重新 prepare/build，不能手填旧 hash。
- P11.3 主 Agent 冻结副本未改。没有启动这个 payload、UAC、SCM 安装或真实卷扫描。

验证命令与结果：

|检查|结果与范围|
|---|---|
|`scripts/dev.ps1 test --locked --lib search_index`|7 PASS：真实虚构文件 + mock client；Scope 外发现但 read/write 拒绝、跨 session 引用拒绝、reset 失效、空默认 roots 可全盘发现；复杂条件保留、失败仅 roots 补充、取消不补充；错误 FRN/同 serial 不同 GUID/恶意 suffix；固定管理参数|
|`scripts/dev.ps1 test --locked --lib file_index::worker`|24 PASS / 1 ignored；ignored 是恢复测试实际调用的崩溃辅助。固定服务生命周期 mock、普通真实 IPC/调用者过滤、取消、快照 receipt、增量/恢复继续通过|
|`scripts/dev.ps1 test --locked --lib coverage_reasons_are_static`|新增 1 PASS；未知错误/路径/SQL 文本只输出静态 metadata_unavailable，并去重|
|`scripts/dev.ps1 test --locked --test file_sessions`|2 PASS；会话/取消/确认隔离及变更对象拒绝|
|`scripts/dev.ps1 test --locked --test dialogue_protocol`|14 PASS；可用性 fallback、取消/拒绝/工具错误不重放、模型无法注入授权|
|`scripts/desktop.ps1 test -CargoArgs @('index_service')`|2 PASS；管理确认/参数拒绝，启用偏好独立于 Scope/config/P9；准备 payload 后再次通过，无提权|
|`npm test --prefix desktop`|42 PASS；包括状态不提权、本地取消/同意及模拟 UAC 取消、缺 payload 禁用安装、代次/时间/静态原因与恶意文本转义|
|核心 clippy `--locked --lib --bin dao-index-worker --tests -- -D warnings`；桌面 clippy `--tests -- -D warnings`|PASS|
|核心/桌面 fmt check|PASS|
|`scripts/prepare-index-payload.ps1`；`scripts/desktop.ps1 build`|release worker + SHA manifest + 嵌入 Debug 桌面 PASS；未构建发布安装包|

收尾：enable 遇已停止/版本不符/未知状态时先给明确操作说明，不盲目 install 或触发无效 UAC。名称发现与操作 Scope 的说明同时更新设置和模型；复杂条件绝不为走服务而删除。取消桥接线程在正常返回和 unwinding 时均停止/join。普通覆盖原因只用静态标签，不传私有总行数。安装/升级仍需要显式设置确认，source bytes 提权前重新核 SHA 并持有文件/祖先句柄；真实 RunAs 行为未验证。

## 未验证与后续边界

- 主 Agent 可使用其隔离配置进行 native 未安装路径试用；本节不冒称 native 实测。不要点击确认启用/服务管理来替代独立授权。
- 实际安装、LocalSystem 服务、完整交互账户 ACL、跨账户、升级/回滚/卸载及重启均 NOT_VERIFIED；本 Task 未重新弹 UAC。P11.2 原实际后续 USN 试验仍未验证。
- 名称搜索首版是各卷有限首屏，回执报告 has_more；复杂筛选/后续页仅列出的目录补充，不承诺全盘所有排序/分页。未知卷/不完整覆盖有明确标签。
- 当前连接预算 3 秒；SCM/image 核验及文件 API 的 OS 调用不宣称硬实时中断。动态卷上线/离线、日志恢复和性能留 P11.5。历史首次 228.6s、更新 15.8s 未达原 30s/2s，查询 p95 也未达 200ms；未放宽目标，未重复同算法全盘测试。
- 打包流程和固定 payload 的构建/嵌入通过；完整发布安装包、签名/干净机器运行未执行。
- Independent testing: NOT_RUN。Independent review: NOT_RUN。交接后停止本 Task，不自行进入 P11.5。

## 主 Agent 补充原生隔离试用

针对冻结 Desktop Debug `7E1E45C33003B49005B91F0B6A33556F4C72B8816B2FD9591CD395E660252F2D`，主 Agent 实际 native WebView 试用 PASS，证据 `.tools/p11-20261013/desktop-trial/native-evidence.json`，候选副本 `.tools/p11-20261013/desktop-candidate/`。启用偏好 false/true 两次实际启动均返回 `not_installed + payload_ready`，未确认 enable 被拒绝，设置显示尚未安装。无模型搜索虚构关键词仅返回 allowed 内两项，未发现 discover-only；来源非 windows_service，启用后明确服务不可用/非全盘目录补充。配置内容未变、窗口已关闭。无 UAC/真实服务。此项补齐前述 native 未安装路径待测项，不能外推为实际服务/全盘查询通过；独立 Tester/Reviewer 未启动。
