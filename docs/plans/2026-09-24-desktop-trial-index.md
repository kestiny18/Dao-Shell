# 桌面首轮试用反馈 — 执行索引

Status: P1 DONE；P2–P5 READY；P6 DISCUSSION_COMPLETE；P7–P8 READY

## Current scope and authorization
- P1实现和历史独立验收已归档并提交；P2–P5已完成原型实现与自测，独立测试/评审未执行。
- 用户明确P6为探索讨论任务，已选择资源压力详情、精简电脑档案加入后续计划，分别为P7/P8；尚未授权实施。
- 当前授权：按最小独立功能分别提交已完成任务及计划文档。不推送、不发布、不开始新功能。
- 当前工作流以docs/workflow/README.md为准：Coder实现与自测、原型交付；独立Tester/Reviewer仅用户明确触发。历史FULL记录只适用于当时版本。

| Plan | 范围 | 验证 | 状态 |
| --- | --- | --- | --- |
| [P1](2026-09-24-p1-workspace-sessions.md) | 工作区、会话、恢复、输入框布局 | 历史FULL验收；后续界面迭代见单独记录 | DONE |
| [P2](2026-09-24-p2-settings.md) | 模型中心配置、目录列表 | Coder自测PASS；独立验证NOT_RUN | READY |
| [P3](2026-09-24-p3-search-fallback.md) | 统一输入与受控搜索回退 | Coder自测PASS；独立验证NOT_RUN | READY |
| [P4](2026-09-24-p4-overview.md) | 概览布局及应用信息 | Coder自测PASS；独立验证NOT_RUN | READY |
| [P5](2026-09-24-p5-app-actions.md) | 应用运行、位置、系统卸载入口 | Coder自测PASS；独立验证NOT_RUN | READY |
| [P6](2026-09-24-p6-exploration.md) | 信息可行性及优先级讨论 | 代码与官方资料探索，未实机采集 | DISCUSSION_COMPLETE |
| [P7](2026-09-29-p7-resource-pressure.md) | 资源压力详情 | Coder自测PASS；独立验证NOT_RUN | READY |
| [P8](2026-09-29-p8-computer-profile.md) | 精简电脑档案 | Coder自测PASS；独立验证NOT_RUN | READY |

## Delivery limits
P2–P5的实现、自测和产物版本详见各Plan；原生视觉、实际系统操作、真实模型、干净机器和安装包的未验证项分别保留，不把原型READY等同完整验收。文件操作自测使用隔离夹具。P5实际启动/UAC/定位/系统卸载页面未实测。

## Historical authorization
最初从3484a37ab14251a46f99a0adfca0c80f818f5ce6开始记录P1–P6，P1交付后提交为2e02e2d。后来曾记录顺序P2–P6及定时续作授权；P6现依用户明确要求修正为讨论，不从旧记录启动开发或定时任务。既有代码和验证事实保留在对应Plan及历史报告中。

## Scoped commits
- P2: 5c133bc — 模型设置及目录选择。
- P3: 0bd33f3 — 统一输入与受控回退。
- P4: ed90183 — 网络卡片及应用信息。
- P5: 465e4cc — 身份核验与应用操作。
- 拆分验证：四阶段DOM分别28/29/30/33通过，四阶段cargo check --locked --tests通过；最终29个相关文件与拆分前内容一致。此为拆分自检，不替代独立验收；临时检查文件不入库。

2026-09-30：用户明确授权执行P7/P8，已完成实现、自测及统一构建。具体证据和未验证项见两份Plan；未提交推送，早期仅规划授权已被本次实施授权更新。
