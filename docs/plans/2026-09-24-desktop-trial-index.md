# 桌面首轮试用反馈 — 执行索引

Status: P1 DONE；P2–P6 PLANNED
Authorization: 用户确认拆分方案，并于本轮授权先记录 Plan、执行 P1。
Delivery: 工作区代码与验收记录；未授权 commit、push、merge、release。
Baseline: 3484a37ab14251a46f99a0adfca0c80f818f5ce6；开始时工作区干净。

按顺序执行，每个 Plan 独立验收。本轮只执行 P1。不得从本索引推导后续 Plan 的启动授权。

| Plan | 范围 | 验证 | 状态 |
| --- | --- | --- | --- |
| [P1](2026-09-24-p1-workspace-sessions.md) | 工作区、会话、恢复、输入框布局 | FULL | DONE |
| [P2](2026-09-24-p2-settings.md) | 模型中心配置、目录列表 | FULL | PLANNED |
| [P3](2026-09-24-p3-search-fallback.md) | 统一输入与受控搜索回退 | FULL | PLANNED |
| [P4](2026-09-24-p4-overview.md) | 概览布局及应用信息 | STANDARD | PLANNED |
| [P5](2026-09-24-p5-app-actions.md) | 应用运行、位置、系统卸载入口 | FULL | PLANNED |
| [P6](2026-09-24-p6-exploration.md) | 电脑信息探索清单 | LIGHT | PLANNED |

共同约束：保持进程内架构，不引入后台 Agent 或通用框架；真实文件操作只用隔离夹具；不迁移历史 Plan；源码、原生体验、安装包验证分别报告。FULL 使用 Coder → 空白上下文 Tester → 空白上下文 Reviewer，绑定最终候选版本。
