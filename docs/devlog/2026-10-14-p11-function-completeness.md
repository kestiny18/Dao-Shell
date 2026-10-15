# P11 F1 — 状态真实性与手动恢复闭环

Implementation: READY（仅本轮 F1 源码与隔离自测范围）
Self-test: PASS
Independent testing: NOT_RUN
Independent review: NOT_RUN
Baseline: ca051de6cb9517b6285a4dbeed33206786c0e956 + 已有未提交 P9/P10/P11 工作区。
Authorization: 2026-10-14 用户要求暂停查询优化，优先功能完整性；主 Agent 派发 F1 状态与异常路径补齐。未授权提交、推送、发布或迁移。

## 修复与边界

- `Catalog::discover` 原来将“仍被发现但不支持”的卷也视为离线，后续发现把 `unsupported_filesystem` / `not_fixed_volume` 等实际原因和运行错误混在一起。增加独立 `present` 状态：只有发现结果确实缺少该身份才标离线；不支持的卷保留原始资格原因、清除无效 pipe，不启动 worker。
- 从受支持变为不支持时取消旧 writer，并继续等待 join。期间晚到的 ready 信号不得恢复 pipe；重新变为受支持时仍等待旧 writer 完全结束后启动。GUID + serial 身份绑定和既有目录/worker 数量上限不变。
- 维护线程退出时保留已发布代次、时间与覆盖原因，但以 `maintenance_stopped` 取代旧的可重建错误，避免退出后还显示成可通过同一 writer 重建。明确的 `storage_identity_changed` 保留，要求检查存储并修复服务。
- 设置页面现在显示 `index_status.error` 的白名单中文解释；未知码只显示固定通用说明，不反射私有错误文本。维护退出或存储身份异常不再显示无效重建按钮。服务版本不匹配时也不显示卷重建按钮。
- 活跃 writer 的 `rebuild_required` 仍可由用户本地确认重建。已退出 worker 给出“停止服务后重新启动”的明确指导；暂时 Journal 不可用保留原有有限自动重试语义。服务停止、断线和版本不符分别指向已有启动、检查/修复、更新入口。
- 没有新增控制协议、自动重启、自动重建或自动提权。没有加入名称快照/缓存、FTS、SQL 加速、采集算法替换或新性能实验；Scope、调用者 ACL 与文件身份验证保持原边界。
- 主 Agent 实机指出取消停止服务时安装器返回“没有继续安装操作”不准确；源码改为“没有继续服务管理操作”，仅改提示，不改变取消状态或控制逻辑。

## 功能闭环检查

| 路径 | 当前实现与本轮结论 |
| --- | --- |
| 桌面启用 / 停用 | 独立 preference 保存；停用保留服务与索引。设置状态读取不会启动管理操作。已有隔离测试覆盖偏好与 Scope/旧目录配置隔离。 |
| 服务停止 / 版本不符 / 断线 | 不自动安装或提权；明确提示已有手动启动、更新、修复入口。本轮 UI fixture 验证只发送 status。 |
| Journal gap / 必须重建 | 仍由存活 writer 通过本地确认、认证卷 pipe 处理；既有去重、30 秒限流、事务发布和源文件不变自测通过。 |
| 维护线程退出 / worker 退出 | 给出手动停止、启动指导；维护已退出时禁用无效重建；保留之前发布信息，不伪装正在更新。 |
| 暂时设备 / Journal 不可用 | 沿用有限退避重试，不将未知错误或 Journal gap 自动变成全量重建。 |
| 升级 / 修复 / 回滚 / 卸载 | 固定命令与既有 SCM 模拟测试覆盖失败停止、pending 恢复、失败健康回滚、保留索引。实机生命周期仍由主 Agent F2/F3 协调；此记录不替代实机证据。 |

## 自测

运行环境：Windows，仓库固定 Rust GNU 工具链与本地 w64devkit；所有文件、IPC、ACL 测试使用独立临时虚构夹具，无模型调用。

- `scripts/dev.ps1 test -CargoArgs @('--locked','--lib','file_index::worker','--','--test-threads=2')`：40 PASS / 0 FAIL / 1 ignored（隔离崩溃子进程 helper，仅由恢复测试调用）。覆盖 catalog 状态、真实普通 IPC、调用者身份、祖先 ACL、确认重建、取消、崩溃恢复与模拟 SCM 生命周期。
- 新增 core 回归：连续不支持卷发现与真正消失/返回；运行 writer 的资格变化、晚到 ready、先 join 后重启；维护退出时保留发布信息并修正不可恢复状态。
- `node --check desktop/ui/settings.js`：PASS。
- `npm test --prefix desktop`：45 PASS / 0 FAIL（当时包含前两项新增 UI 测试）。随后新增退出 worker 提示用例及完善状态防护，对最终受影响设置 / 回执用例定向复测，结果见下。
- `node --test --test-name-pattern='whole disk settings|whole disk receipts|volume rebuild|volume recovery|stopped incompatible|exited volume' desktop/tests/ui.test.mjs`：最终受影响用例 6 PASS / 0 FAIL。覆盖白名单错误、不泄露虚构私有路径、代次保留、可重建与已退出区别、停止/断线/版本不符只读轮询、临时 Journal 与不支持卷显示。
- `scripts/dev.ps1 clippy -CargoArgs @('--locked','--lib','--tests','--','-D','warnings')`：PASS。
- `scripts/dev.ps1 fmt --check`：PASS。
- `scripts/desktop.ps1 build`：PASS，1m17s。主 Agent 要求增量构建普通 Debug 客户端，继续嵌入现有 04B worker payload；未执行 prepare-index-payload。

普通权限 UI 候选冻结在 `.tools/p11-20261014/f1-debug-client-candidate/dao-shell-desktop.exe`，SHA256 `2C78C8CB15309D65F65149D8E2A71AE720DCCB6DD24430EE250CB4D516E41210`，相邻附带 WebView2Loader.dll。它含全部本轮功能与 UI 修复，但构建发生在最后一条 UAC 取消纯文案调整之前。嵌入 worker SHA256 仍为 `04B9986FD2A0F625D88EE9A94C5CE8FF6AA4240C86D663EA3534D46C910316DB`；这个 Debug 客户端用于与现有服务配对查看新 UI，不能冒称已包含新服务端修复的统一候选。最终统一构建由主 Agent 另行协调。

主 Agent 随后对该 2C Debug 客户端完成普通权限原生集成自测（非独立验收）：现有 04B 服务认证与设置状态为 running；实际 UI 函数对白名单 `maintenance_stopped` 和代次 42 的显示正确，未知虚构私有错误不透传；停用无需提权、返回 enabled=false，兼容查询 source=scan 且找到 1 个虚构结果；重新启用已运行服务无需提权，恢复 source=windows_service 且找到 1 条结果。SCM PID 27452 未变，Scope 配置字节未变，测试自己的窗口已关闭。证据：`.tools/p11-20261014/live-service-trial/f1-native-functional.json`。此检查验证新客户端/UI 与旧服务配对，不证明新服务端 F1 已部署。

## F3 — 统一候选构建（不安装）

主 Agent 明确派发只构建统一 worker、Release 桌面和 NSIS 安装包；禁止运行安装、升级或其他 UAC 管理。现有旧安装包 SHA256 `A9D31DC4D165A4E834D2E6F70904A64CF96735F1F9AE542E3A3D148E87E8C8F6` 已连同原 `.sha256` 和旧 payload manifest 保存在 `.tools/p11-20261014/function-unified-candidate/previous-A9D31DC4D165A4E834D2E6F70904A64CF96735F1F9AE542E3A3D148E87E8C8F6/`，校验副本与原件一致。

使用正式 `scripts/package-desktop-windows.ps1`：PASS。显式 prepare 最新固定 worker / SHA manifest，再构建 Release 桌面与 NSIS。worker Release 构建 41.77 秒，桌面 Release 构建 2m45s；这些仅为构建耗时，不是搜索性能实验。来源为上述 HEAD + 本轮记录的未提交工作区，包含最后的 UAC 中性取消文案。没有运行安装包、服务命令或提权操作。

产物已冻结至 `.tools/p11-20261014/function-unified-candidate/`：

| 产物 | SHA256 | 字节 |
| --- | --- | --- |
| `dao-index-worker.exe` | `F5882D1CA4195DBD511EC9B057F0EE38B771A83841E7D0947A8340D9D46668B1` | 3918336 |
| `dao-shell-desktop.exe`（Release） | `43FC94E422227F14BBF395D06993D4F4F0E2BEA807F6B92491CBDD538ECE695B` | 35582976 |
| `WebView2Loader.dll` | `8427B1FC58EC707813E5C0A51EB5D69397BB333250A7B891BE4D3B123F1E0F1C` | 160320 |
| `Dao-Shell-0.1.0-windows-x64-setup.exe` | `B18CC65D14B0BB599A7C9432DA54BB0BCAD29D0F3A21391732E9FBA71FAB84AC` | 224145432 |

固定产物校验脚本 `verify-artifacts.py` 结果 PASS：根目录 Release worker 与 prepared payload 完全一致，manifest 的 SHA 与字节数一致，Release 桌面 EXE 内含完整 worker 字节及相同预期 SHA，桌面 PE 为 x64 / Windows GUI 子系统；dist `.sha256` 正确，所有冻结副本逐字节一致。完整结果写入 `artifacts.json`；`pre-build.json` 保存来源 HEAD 与旧产物信息，`scoped-source-hashes.json` 保存 F1 修改和打包依赖的局部源码哈希（明确不是整个工作区快照），`source-status-before.txt` 和 `package-build.log` 保存构建时状态及命令输出。旧 A9 安装包及其原校验文件、manifest 仍保留。

打包资源目录顶层仅有 LICENSE、试用说明、WebView2Loader.dll 与第三方许可目录。本轮冻结仅复制上表产物、安装包 `.sha256` 和公开 payload manifest，不复制私人配置。构建与校验不等于 NSIS 实机安装或新服务功能已验证。

新统一桌面预期 SHA 为 F588，而已安装服务仍为 04B。主 Agent 已对冻结 `43FC94…` Release 完成普通 native 集成自测 PASS：实际状态为 version_mismatch，设置显示“服务管理中更新”指导且无卷重建按钮；直接 enable 在本地被拒绝，未调用提权；虚构关键词使用 source=scan 的Scope兼容目录fallback返回1条；SCM PID/ImagePath与配置字节未变，自己的窗口已关闭。证据 `.tools/p11-20261014/live-service-trial/unified-native-version-mismatch.json`。这是与原服务版本差异的兼容功能证明，不是新服务端部署证明。新版服务部署、修复、回滚或卸载仍需用户之后的明确管理指令；已取消的 service-stop 提权不在此轮重试。

## 版本与未验证项

本轮修改范围：`src/file_index/worker/service/recovery.rs`、`src/file_index/worker/mod.rs`、`src/file_index/worker/tests.rs`、`src/search_index/install.rs`、`desktop/ui/settings.js`、`desktop/tests/ui.test.mjs`、`desktop/README.md` 与本记录。主 Plan 和实机 devlog 由主 Agent 维护。

Coder 本轮未操作真实 SCM、UAC、系统 Journal 或源用户文件。F3 新 worker/桌面/安装包已构建并另存，旧冻结产物与现有运行服务保持不变。现有实际运行 04B 服务不含本轮服务端修复；新服务端行为仅有隔离自测证据。统一候选的实机部署、真实服务重启、升级/回滚/修复/保留索引卸载、物理卷恢复和主机重启仍需要主 Agent 继续协调。本记录不宣称 P11 整体完成或独立验收通过。
