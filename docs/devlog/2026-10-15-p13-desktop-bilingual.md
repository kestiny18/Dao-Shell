# P13 — 桌面中英文实现记录

Plan: [P13](../plans/2026-10-15-p13-desktop-bilingual.md)
Baseline: ca051de6cb9517b6285a4dbeed33206786c0e956 + 已有 P9/P10/P11/P12 与 embedBootstrapper 全部未提交改动，保留。
Implementation: READY
Self-test: PASS
Independent testing: NOT_RUN
Independent review: NOT_RUN

## T1 — 系统语言与保存

- Config 新增 ui_language，合法值 auto / zh-CN / en；旧配置缺字段默认 auto，读取不落盘。Windows 读取 GetUserDefaultUILanguage，不使用地区/时区/日期格式推断。LANGID 中文 primary language（包含简繁及香港等）统一简中，其余或未知英文。
- 显式选择优先；原有配置同步、原子替换与 expected_revision 检查继续执行。保存响应提供 effective_language，只有成功后切换；不会为切换语言重放请求或确认。模型目录、密钥引用、访问模式、读写目录、主题继续原样保存。
- 只读 language_info 用于首屏；前端先解析语言再揭示页面。读取失败揭示英文与可见提示；无自动保存或模型调用。旧 CLI 默认中文交互保留。

## T2 — 显式文案与数据边界

- i18n.js 集中内置消息键和插值；HTML 只翻译 data-i18n 明确标记，动态代码显式调用 t。没有 DOM 全局中文替换、MutationObserver、在线翻译或任意用户数据扫描。
- 覆盖导航/重命名、概览/容量/双榜/应用/网络/档案、设置/模型/目录/索引/诊断/服务管理、过程回执、确认/取消/保存/错误。后端-owned 提示字段通过明确内置消息匹配；错误已知权限/取消/过期/冲突等给英文说明，未知失败仍报告未完成。诊断 JSON 和协议代码稳定。
- 用户文字、自定义标题、路径、应用/设备名称、现有消息和模型原回复不翻译；历史 receipt 正文保留原文，摘要/外壳使用当前语言。缓存概览/档案在成功切换后重新渲染，不重新采集或自动模型调用。
- 安装器 English 在首位作为非支持语言兜底，仍提供 SimpChinese；保持 embedBootstrapper。新增英文 TRY-IT 指南并加入打包资源。

## T3 — 新请求语言与检查点

- FileSession 新增显式 language 参数入口，原默认构造保持中文以兼容 CLI。桌面依据保存配置解析语言。
- 桌面模型 system 指令替换原强制中文规则，按当前语言简洁回答，用户明确指定其他语言时优先；工具能力/授权/数据不可信/事实/确认与全盘发现规则保留。重置仍保留语言规则，无自动模型请求。
- 语言分流纯函数 1 PASS：中文 CN/TW/HK/SG/MO，英文/日德/未知，显式覆盖。
- UI 58 PASS：原 54 回归及 4 语言检查，覆盖保存/重启/auto/失败不切换、用户中文内容保留、首屏失败英文可用、错误和缺键/两语检查。后续微小文案与 HTML option 标记纠正需最终再跑。
- 共享 settings_and_access 5 PASS、dialogue_protocol 15 PASS：旧配置只读/持久化/冲突/写失败保留、模型/凭据引用/主题不丢；本地模拟模型捕获两语 system 指令、无冲突中文规则、用户法语请求原样、能力边界与 reset/global-discovery 后语言保留。
- 桌面 Rust 19 PASS，包含原候选/配置/确认/请求互斥/组件诊断边界。
- P12 原 9,072,494-byte 小包 SHA D6F32284222E7B5CFC34ABD9DDB6DCE7D9437AAC7BA4210374D2476B026F5076 已冻结 .tools/p13-20261015/final-candidate/previous-p12-bootstrapper-setup.exe；224MB offline 包保留。

尚未运行本轮 Debug/Release、NSIS 或原生隔离界面。最终原生由主 Agent 检查实际中文系统 auto、显式英文/保存重启/auto 和英文宽窄布局；实际非中文 Windows 系统未验证，以注入纯函数/DOM 检查覆盖分支。未管理系统服务或请求 UAC，未操作真实用户文件/既有进程，未提交/推送/发布。P11 生命周期缺口、另一台电脑 C/E 持续失败 NOT_REPRODUCED 不因双语改变。


### 最终源码检查 / Debug

- UI 最终 58 PASS（含真实解析后 option 英文标签及 HTTP401 指导）；core settings 5 / mock dialogue 15 / Windows语言分流1 / desktop Rust19 均 PASS。
- core lib/tests 与 desktop all-targets clippy -D warnings，core/desktop fmt --check，i18n/app/settings/home JS语法均 PASS。
- prepare worker Release：SHA256 E058A698A4F9C44A5A44F0107C42708188D3E29FFA775B56E052B101CAF47219，3,943,936 bytes，仅编译未安装。旧实机服务仍04B。
- Debug Jobs2构建 PASS。冻结 `.tools/p13-20261015/final-candidate/debug/dao-shell-desktop.exe`：342,771,200 bytes；SHA256 59607028DC40B60FDFEF0B1CCFFD87BEC16E36309CE6FC969D8D4E6F5695EFC3。同目录 GNU loader160,320与匹配manifest、artifact.json齐全。已交主 Agent 隔离原生试用，未由Coder重复操作。
- 源码在本次Debug后冻结；Release/NSIS打包进行中。必要原生finding由主 Agent 返回后再顺序修改与复验，不把正在进行的检查提前记为通过。


### Release / 安装包产物

- `scripts/package-desktop-windows.ps1 -Jobs 2`：PASS，Release + embedBootstrapper NSIS 构建成功；未执行安装包、组件管理或 UAC。
- 安装包 `dist/Dao-Shell-0.1.0-windows-x64-setup.exe`：9,100,976 bytes（约9.10 MB / 8.68 MiB），SHA256 `25136DED13FF18FAE8195FA2E21445EC0D0C4D70EFB2B050A216E22920237071`，旁边 .sha256 与实际字节一致。
- Release desktop：35,801,088 bytes，SHA256 `ACE1F68329054EF504605AA8920BF3BC51D75E98F24BE91E78E05D62BE835698`；payload E058A698（3,943,936 bytes）完整字节及 SHA 嵌入匹配。
- `.tools/p13-20261015/final-candidate/verify-package.py`：PASS。检查小型 WebView2 bootstrapper1,860,304 bytes、已有运行环境检测/失败中止、两语指南同源字节及 NSIS资源、English首位兜底、GNU DLL160,320、x64 GUI、worker/manifest、checksum与冻结副本。原P12 D6F322小包及B18CC离线包原样保留。
- 冻结安装包/Release桌面/DLL/manifest/installer.nsi/checksum/artifacts.json及package.log在上述final-candidate；Debug目录及其独立SHA已列在前段。package生成manifest JSON字段顺序可能与Debug副本不同，解析后的schema/digest/bytes相同；worker字节digest相同。
- 实际安装、缺少WebView2的联网引导/离线失败、真实服务更新以及非中文Windows物理环境仍NOT_VERIFIED。主Agent原生双语检查尚待其结果；本段不提前宣称整个P13 READY。


### 主 Agent 原生 v1 / 窄文案 v2

- 主 Agent `.tools/p13-20261015/native-trial/language-evidence.json`：PASS，对应冻结Debug `59607028...`。实际中文Windows用户UI（LANGID2052）默认zh-CN；草稿选择英文不立即生效，成功保存后英文；重启保留英文；切回auto恢复中文。自定义中文标题、历史文字及含中文候选路径保持原文；实际无模型查找虚构文件返回1候选，未打开文件。英文宽1647/内容1361与窄747/内容481均无文档横向溢出，自己的窗口已关闭。该证据由主 Agent 集成自测产生，不是独立验收。
- 源码补查发现：少量Inventory现场扫描说明未逐条映射，可能把成功扫描的英文提示错误回退为“操作未完成”。只补现有Inventory状态/覆盖说明的内置译文，并区分未知status（明确详情不可用、不能确认覆盖或成功）与error（明确操作未完成）；动作、协议、Rust配置/语言规则均未变。新增定向回归后UI全59 PASS，JS语法PASS；没有因纯文案变化重复Rust测试。
- v1 Debug/安装包/证据均保留，未覆盖冻结副本。最终v2 Debug在 `.tools/p13-20261015/final-candidate-v2/debug/`：342,772,224 bytes，SHA256 `C4484A87A04138106469236C07408A54CF1E6E666C6863C22DCFC3F3687EDD4D`；GNU DLL160,320和同一E058 worker manifest齐全。
- 最终v2 Release/NSIS构建及verify-package全部PASS；安装包9,105,022 bytes（约9.11 MB / 8.68 MiB），SHA256 `09539E59A012DFAB5CD9DAF28A71D4BBB5A52A72A4F6F74B42534D203B7C628A`，dist旁边checksum一致。Release桌面35,802,112 bytes，SHA256 `C5D143FF77CE814F328475A9900B1E0D0768F9EF616C415336E3D0FA955ABC03`。冻结目录final-candidate-v2包含全部产物、包源NSIS/manifest、校验、artifact.json与package.log；P12旧小包与B18CC离线包保留。
- v2对主 Agent 当前大轮证据只影响非error状态/现场扫描文案分支。已交新版Debug与该finding，请主 Agent补一次真实英文受限搜索的coverage_note证据；未改的配置保存/重启/布局/用户数据边界可以明确沿用v1相应检查。该最后集成结果待主 Agent补充，不提前记为PASS或宣布整个Plan完成。

Coder交接：T1/T2/T3实现、自测、两次必要候选构建与本地小包产物已完成。Independent testing/review仍NOT_RUN。当前产品源码为v2，未提交、推送或发布；真实服务未管理，另一台C/E故障与P11缺口保留。主 Agent决定最终Plan状态。


### 最终 v3 — 主 Agent 文案 finding 闭环与交付

- 按主 Agent 本轮明确finding，英文计数统一采用无复数句式：Records: N、Known running: N、Items: N、Search record · Items: N、Skipped items: N、Hours: N、Logical CPUs: N，以及资源/已确认实例计数；CPU窗口用s单位。FileSession英文结果使用Matches: N，中文结果和CLI默认行为不变；没有引入plural框架。
- language.note明确仅在“跟随系统时 / When following system”按系统分流，手动选择优先，保存后生效。没有继续扩大润色范围。
- 自测：UI全量60 PASS；FileSession3 PASS（含0/1/2条英文匹配及中文文件名保留）；core lib/tests clippy -D warnings与fmt、i18n语法PASS。此前shared settings5 / mock dialogue15 / 语言分流1 / desktop Rust19与desktop clippy/fmt，未改相关边界，按注明范围沿用。
- 明确重新prepare worker，最终SHA256 `90E1FC4AB8039B72FECF5FE87E37D6043FA365A896C36144DA179598187785F9`，3,943,936 bytes；未沿用v2 manifest或安装实际服务。最终Debug：342,772,224 bytes，SHA256 `ED9761B9773AFCFEA1BF52E3BF1219168FBBC8316159757CD1F0492A3A0EF5D3`，冻结final-candidate-v3/debug，同目录GNU DLL与匹配manifest齐全。
- 主 Agent最终受影响原生补测 `.tools/p13-20261015/native-final-text/native-text-evidence.json`：PASS。准确对应ED9761 Debug；实际English restricted搜索error=false、Matches:1、现场扫描未登记提示真实到达UI且不误报操作失败；2个回执默认折叠，中文文件名/路径保留。Records:1、Known running:1、Hours:1和手选优先说明正确；自己的配置未修改/窗口已关闭；english-live-search.png由主 Agent目视正常。v1未改语言保存/重启/auto/用户历史/英文宽窄流程，明确按v1版本沿用前述证据。
- 最终v3 Release/NSIS构建及冻结verify-package：PASS。安装包 `dist/Dao-Shell-0.1.0-windows-x64-setup.exe`：9,101,478 bytes（约9.10 MB / 8.68 MiB），SHA256 `8A180F1B52B34D0D574B662B63F19B9458445CC42AE81B79619BFED9DA83C42A`；旁边checksum匹配。Release桌面35,802,112 bytes，SHA256 `38B78EBEA80059A10C9F2A75065D496DB3798AAE3BDA47F315A3F689E99075E3`。
- 最终小包检查确认embedBootstrapper1,860,304、已有runtime检测/失败中止、英文/中文两份指南、English首位兜底、GNU DLL160,320、完整90E1 worker字节/摘要、x64 GUI、旧P12及B18CC offline包保留、SHA及冻结副本。最终产物、manifest/installer.nsi、checksum、artifacts.json、verify脚本及package.log位于 `.tools/p13-20261015/final-candidate-v3/`；v1/v2所有冻结证据保留。

本轮双语原型实现及所需自测/本地包/主Agent原生集成完成，建议主 Agent标记P13 Implementation READY / Self-test PASS。独立测试与评审均NOT_RUN。实际非中文Windows物理环境、真实模型回复质量、干净机器和缺Runtime安装未验证；系统语言非中文分支由注入纯函数覆盖。真实服务仍旧04B未管理，另一台C/E失败NOT_REPRODUCED及P11实机生命周期缺口保留。无提交、推送、发布或历史迁移；不从本轮完成推导P11自动任务完成。
