# P13 — 桌面中英文与系统语言默认值

Implementation: READY
Self-test: PASS
Independent testing: NOT_RUN
Independent review: NOT_RUN
Authorization: 2026-10-15 用户为对外发布要求中英文、中文系统默认中文其余默认英文、允许用户修改配置。进入桌面功能实现与自测，准备本地双语试用包；公开发布渠道/版本未确定，不从发布意向推导上传或Git操作权限。
Baseline: ca051de6cb9517b6285a4dbeed33206786c0e956 + 当前全部P9/P10/P11/P12及embedBootstrapper未提交改动，全部保留。
Delivery: 双语桌面原型、配置兼容、自测记录与本地小型安装包。

## 目标与范围

- 桌面选择“跟随系统 / 简体中文 / English”。旧配置无语言字段等同跟随系统；Windows用户界面语言属于中文（包括zh-CN/zh-TW/zh-HK）显示简体中文，其余或无法识别显示英文。使用系统用户界面语言而非地区、时区或文件名。
- 显式选择优先于系统语言，保存后应用到界面并跨启动持久化；切回跟随系统恢复系统规则。配置继续沿用已有原子保存与版本冲突机制，不丢失模型、密钥引用、权限或主题。
- 完整覆盖桌面静态/动态文案、按钮/标题/占位符/无障碍名称、概览资源与应用、档案、文件搜索回执、权限/取消/确认/错误提示、模型设置/测试、索引状态与诊断入口、会话与持久化提示。英文布局不截断关键操作。首屏避免未解析语言时闪出整页错误语言。
- 使用集中、明确的消息键和插值；不得通过全局DOM中文字符串替换或观察器翻译任意内容，不把用户文本/路径当文案键。缺键测试和明确英文回退，不为消除中文把真实异常伪装成正常。
- 用户输入、文件路径/应用/设备名称、会话自定义标题、已有聊天记录和模型原有回复保持原文。新建默认标题按当前语言。诊断JSON字段与安全错误码保持稳定，不翻译协议值或扩展收集私人信息。
- 新桌面模型请求按当前语言指导回答，用户明确指定语言时尊重用户；仍保留工具能力、权限、事实与确认边界。不要自动发模型请求或调用在线翻译，测试使用模拟响应。
- 本轮面向桌面；CLI交互文本与仓库文档全量翻译不展开。必要共享配置/桌面会话/模型提示可窄修改，CLI原默认行为保持兼容。安装器已有中英文，核对英语兜底与双语体验指南，保留小型WebView2引导模式。

## Tasks（单Coder顺序执行）

### T1 — 系统语言与配置选择
Implementation: READY（配置自测与原生保存/重启/auto通过）
验收：旧配置兼容、中文/非中文/未知系统语言、显式覆盖、切回系统、保存/重启、保存失败不假生效、并发修改冲突、关键设置不丢失；当前请求/确认不得因语言切换重放或绕过。

### T2 — 桌面完整双语
Implementation: READY（双语文案、相关自测、英文原生宽窄与finding闭环）
验收：所有桌面内置用户文案及状态覆盖两种语言，用户内容不改；英文首屏、设置、概览与搜索/确认/异常/恢复主要路径可用，长英文布局无关键按钮遮挡；主要桌面错误显示可理解的当前语言，未知错误有诚实回退。

### T3 — 新模型回复语言、集成与试用包
Implementation: READY（模拟模型语言、原生集成、双语指南与最终小包）
验收：模拟模型捕获所选语言指令，旧历史不翻译且无自动请求；中英配置相关核心/桌面/UI检查及隔离原生试用；最终Debug/Release与小型NSIS包、SHA和来源记录，保留P12旧包。

## 约束与验证方式

- 主Agent负责范围、调度、Plan与交付；一般功能由唯一Coder修改，自测不冒充独立验收。用户没有要求独立测试/评审，均NOT_RUN。
- 不自动安装/更新/停止索引服务、不重试取消的UAC、不重启电脑、不做性能优化/索引迁移；P11实机缺口、另一台C/E持续失败仍分别保留。
- 只用独立配置、虚构文件和模拟模型；原生只读界面或自建夹具，不能操作用户真实文件/进程。先完成源代码检查与相关测试，最后一次统一构建打包，不机械重复未变化检查。
- 结论注明工作区候选、覆盖范围及未验证项。另一台Windows“基本可用”为用户试用反馈，不能替代本轮双语验证或原有完整性缺口。

## 执行记录

- 开始时HEAD仍ca051de6，无暂存改动；当前未提交P9–P12范围全部保留。桌面HTML固定zh-CN，动态JS与FileSession/模型提示存在中文硬编码；安装器已有SimpChinese/English，需核对英文兜底。按当前工作流派发单Coder完成T1–T3与自测。
- 接口：Config.ui_language三值；只读language_info及session_info返回effective_language，get/save_settings响应也含effective_language，保存成功按该响应应用，避免第二次读取造成假失败。前端#ui-language、applyLanguage、languageReady；376条集中消息键/插值，明确data-i18n节点，不扫描用户内容；解析失败揭示可用英文界面。
- 检查点：系统分流纯函数1 PASS（中文各地区/英文日德未知/显式覆盖），UI58 PASS（既有54及语言选择、失败/恢复/首屏/用户内容/消息覆盖）。共享配置与模拟模型、最终原生和打包尚未完成，当前不宣称就绪。本机只读原生LANGID2052，对应跟随系统中文；其他系统语言由纯函数夹具检查，不冒称真实英文Windows已验。
- v1实现方检查：共享settings5、mock dialogue15、desktop19、UI58、clippy/fmt/JS PASS。Debug SHA59607028DC40B60FDFEF0B1CCFFD87BEC16E36309CE6FC969D8D4E6F5695EFC3；主Agent原生隔离流程PASS：显式中文→草稿不切换→保存英文→重启英文→auto恢复本机中文，权限/模型保留，中文自定义标题/历史/路径不变；实际英文无模型搜索找到虚构中文名文件；英文已有app/network/resource/profile/model说明无中文残留；英文设置与宽窄截图目视正常，窄内容clientWidth/scrollWidth均481，自己的窗口已关闭。证据`.tools/p13-20261015/native-trial/language-evidence.json`，为自测而非独立验收。
- 收尾finding：Coder发现少数Inventory现场扫描提示漏翻译，未知非error状态误用失败回退，已补35条与非error/失败分流回归（UI59 PASS，v2候选冻结）。主Agent英文原生目视另见“1 records / 1 known running apps / Found 1 candidates”语法及language.note未明确仅auto规则，交Coder一次窄文案修正为中性计数/明确手动覆盖，不引入复数框架。最终以v3准确产物和受影响英文现场搜索补测交付；v1未改配置/历史/布局路径证据注明版本后沿用，不能把旧证据冒充新分支已验。
- v3窄修正：UI60 PASS、FileSession3 PASS（0/1/2 Matches计数与中文路径）、core clippy/fmt/JS PASS；新worker90E1FC4AB8039B72FECF5FE87E37D6043FA365A896C36144DA179598187785F9，准确Debug ED9761B9773AFCFEA1BF52E3BF1219168FBBC8316159757CD1F0492A3A0EF5D3。主Agent受影响原生英文实搜PASS：Matches:1、Live scan覆盖提示正常、不误报操作失败，Records/Known running/Hours中性计数与手选优先说明正确；当前候选中文路径保留、回执默认折叠、配置未变且窗口关闭。证据`.tools/p13-20261015/native-final-text/native-text-evidence.json`，截图目视通过。其余未变语言配置/历史/宽窄路径沿用注明版本的v1证据，无进一步产品finding；最终v3安装包校验待交接。

## 最终交付

- 本轮范围Implementation READY / Self-test PASS；独立测试、独立评审均NOT_RUN。最终UI60、FileSession3、settings5、mock dialogue15、语言分流1、desktop19及相应clippy/fmt/JS检查通过；原生v1完整路径及v3受影响文案补测按前述准确版本/未变路径沿用。详细命令与finding处置见[实现记录](../devlog/2026-10-15-p13-desktop-bilingual.md)。
- 最终v3安装包`dist/Dao-Shell-0.1.0-windows-x64-setup.exe`：9,101,478 bytes；SHA256 `8A180F1B52B34D0D574B662B63F19B9458445CC42AE81B79619BFED9DA83C42A`。主Agent实际核对dist与checksum一致；Release desktop `38B78EBEA80059A10C9F2A75065D496DB3798AAE3BDA47F315A3F689E99075E3`，完整90E1 worker嵌入。产物冻结`.tools/p13-20261015/final-candidate-v3/artifacts.json`，来源仍ca051de6 + 全部保留的P9–P13未提交改动。
- NSIS embedBootstrapper、English兜底、两语指南、GNU DLL、x64GUI、worker/manifest/校验及P12旧包/B18CC离线包保留检查PASS。此前v1/v2证据原样保留，不作为最终下载包。
- 操作入口：设置→通用与外观→界面语言，提供跟随系统/简体中文/English，保存成功后生效并跨启动保留；旧配置默认auto。中文系统按用户UI语言判定，与地区/时区无关。用户内容和历史不翻译，新模型请求按选定语言指导回答，用户明确指定其他语言时尊重用户。
- 实际非中文Windows物理环境、真实模型回复质量、干净机器及缺WebView2引导安装未验证；非中文/未知系统分支以注入纯函数覆盖。CLI文本没有全量翻译。实际服务未升级或管理、无UAC/真实文件打开或进程终止/历史迁移。另一台C/E持续失败NOT_REPRODUCED及P11实机缺口保持，不据此删除P11自动任务。
- 本地双语原型及包可试用，未提交/推送或公开上传发布；当前未授权具体发布渠道/版本。完成本轮后停止，不扩展产品或发布工作。
