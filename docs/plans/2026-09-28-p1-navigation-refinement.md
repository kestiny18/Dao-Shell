# P1 后续 — 会话列表与标签简化

Implementation: READY
Self-test: PASS
Independent testing: NOT_RUN
Independent review: NOT_RUN
Authorization: 用户要求实施三项界面调整；未触发独立验收。
Delivery: 工作区原型；不提交、推送或发布。
Baseline: HEAD 47835933c482df1aabbfec105e7bc00822fadcd5，加现有未提交 P1 实现及文档。保留全部已有改动。

## Scope / Requirements
- 会话呈现普通文本列表，悬停高亮并显示重命名入口；键盘聚焦同样可发现入口。无删除入口；本轮不删除历史数据，不扩展后端清理。
- 重命名可编辑、保存和取消；非空合理长度；更新列表与标签并沿用工作区持久化；保存失败明确提示且不虚报成功。
- 暂时隐藏折叠功能，历史 collapsed=true 也按展开列表展示，保留左右调整宽度。
- 标签紧邻排列；可关闭标签仅在悬停/键盘聚焦时显示关闭入口，不产生布局跳动。概览仍不可关闭。
- 图片只作为列表视觉参考，不增加置顶/归档等功能。

## Self-test / Trial
- Coder 执行受影响 DOM 回归、JS 语法及 diff 检查；新增重命名保存/取消/失败/恢复的必要验证。
- 使用虚构会话与模拟 IPC。桌面构建只执行交付所需一次，记录产物来源和哈希。
- 独立测试和评审未启动；历史 P1 验收仅适用于当时 candidate-03，本次受影响界面尚未独立复验。

## Non-goals
- P2–P6、真实文件操作、历史迁移、发布；不重写历史验收结论。


## Delivery evidence
- 适用版本：上述 HEAD，加已有 P1 未提交实现及本轮改动；历史 candidate-03 不代表本轮界面已独立验证。
- 本轮文件：desktop/ui/app.js、index.html、style.css，desktop/tests/ui.test.mjs、desktop/README.md；无 Rust/schema 改动。
- Coder 自测：npm test --prefix desktop 28/28 PASS；node --check desktop/ui/app.js PASS；git diff --check PASS（行尾提示）；scripts/desktop.ps1 build 一次构建 PASS。
- 重命名行内编辑，Enter 保存、Esc 取消；trim 非空、UTF-8 512 字节上限；失败恢复原名并显示错误，保留输入供重试。列表与标签同步，重启保留；请求事件不打断编辑。
- 历史 collapsed=true 加载为展开；标签关闭按钮保留占位以免悬停跳动。删除 UI 入口移除，底层旧机制未扩展或操作。
- 产物：desktop/src-tauri/target/debug/dao-shell-desktop.exe；SHA256 CB5C63B494AAC55377FE08DED060F638DAE6AB61E847CF4622B1CEE128D25D9F。
- 试用：依 desktop/README.md 本地 fixture 和 DAO_SHELL_DESKTOP_CONFIG 隔离配置启动该 debug 产物，检查列表悬停、重命名与标签关闭。
- 未验证：本轮原生窗口视觉、真实 IME、安装包。独立测试和评审 NOT_RUN；历史 P1 受影响界面结论对当前版本为 INCOMPLETE（待用户触发复验），不改写历史报告。
- 无提交、推送或发布；P2–P6 未启动。

## Follow-up — unused code cleanup and Git delivery
Implementation: READY
Self-test: PASS
Independent testing: NOT_RUN
Independent review: NOT_RUN
Authorization: 用户明确要求检查并删除无用代码、提交和推送当前工作。
Scope: 清理本轮已撤下入口对应的无调用实现、注册、权限及过时测试；保留仍有运行或旧工作区兼容用途的代码。自测通过后提交本次 P1、导航调整及已授权 Plan 文档并推送 origin 当前 main；不发布安装包。


Cleanup result:
- 删除无调用的会话删除前端分支、bridge 方法、Tauri 命令/注册/权限，旧桌面 reset 映射和 showPage 包装，以及 scope-block/roots/preview/file-icon 等废弃样式。
- 保留旧 collapsed schema 兼容和核心 CLI Reset；取消、确认归属、请求隔离、失败恢复仍保留。
- 删除取消功能对应的6种删除竞态测试，保留6种关闭竞态；最终 DOM 22/22、bridge 2/2、JS 语法、desktop fmt、Clippy、diff检查和build全部通过。测试数量变化来自删除已取消功能用例。
- 最终debug产物 SHA256：79926EB0CD7023A0EEBCA1DF0B683BE77EB0894BD2D7FE64148DC68390315D5D；来源为本轮清理后的工作区，替代上节构建。本轮无原生视觉检查或独立验证。
- 用户本轮授权提交与推送，覆盖P1实现、导航改进、清理及此前已授权记录的计划；历史报告保持原版本限定，不代表当前独立验收结果。
