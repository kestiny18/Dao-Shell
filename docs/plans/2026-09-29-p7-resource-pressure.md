# P7 — 资源压力详情

Implementation: READY
Self-test: PASS
Independent testing: NOT_RUN
Independent review: NOT_RUN
Authorization: 用户明确执行P7、P8，按顺序实现与自测；独立验证未触发。
Delivery: 工作区原型；不提交、推送或发布。

## Goal / Scope
让用户看到当前CPU、内存压力及主要占用者，能依据同一次采样查看详情或主动请求解释。

## Prototype requirements
- 在现有概览资源信息基础上提供CPU、内存占用进程详情与排序；复用既有采集能力，不新增常驻监控服务。
- 显示采样时间、采样窗口、单位及覆盖限制；CPU口径清楚，不能混合不同时间的样本比较。
- 有限权限、进程退出、读取失败显示未知/不可用，不伪造零值，不影响其他信息展示。
- 将观测事实与原因推测分开；一次高占用不能直接断言程序导致卡顿。若引入阈值，需解释含义，不声称综合健康评分。
- 主要信息无需模型可用；用户主动选择解释时才调用已有模型入口，并明确提交的事实范围。
- 手动刷新/页面内按需采样；隐藏页面不无限后台采样，不把进程历史持续持久化。

## Non-goals
结束进程、自动优化、自动清理、进程服务管理、长期趋势、第三方监控依赖、全系统故障诊断。

## Self-test / future validation
实施时以模拟采样覆盖高/低占用、未知值、进程退出、刷新失败、排序、CPU口径和无模型路径，验证采样不会阻塞界面或破坏已有请求互斥。使用隔离配置。
独立测试/评审仅用户明确触发；原生展示、真实采样覆盖与实际性能开销需分别记录，不以模拟测试代替。

## Open questions
具体布局和首屏展示数量在实施时根据现有概览确定；不影响当前计划范围。

Baseline: 42d95d44cc8d89a45bd661dd597eecf5bc558396；开始时工作区干净。

## Delivery — 2026-09-30
- Implementation READY / Self-test PASS；Independent testing/review NOT_RUN。
- 概览同次采样生成CPU/内存各前10名，先排序后截断，明确整机CPU百分比、采样窗口、未知值和采样期间消失数量。模型解释同步使用两组可见数据，只有明确点击才发送，不后台轮询或持久化进程历史。
- Coder自测收尾：resources:: 3/3；最终含P8的DOM37/37；desktop10/10（另定向锁断言1/1）、无模型统一入口1/1；核心/桌面fmt与all-targets Clippy通过，JS语法和产品diff检查通过。
- 版本为HEAD42d95d44cc8d89a45bd661dd597eecf5bc558396加当前P7/P8未提交代码。最终desktop build成功；EXE SHA256 9971F77B83D18317694CA550245D2498D804C8F287E7F81ECA2CD28F7118C5B7，主Agent核对一致。
- 原生采样、视觉与实际采集性能未验证。使用隔离配置和模拟样本自测，不等同独立验收。未提交推送。
