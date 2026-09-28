# P3 — 统一输入与搜索回退

Status: PLANNED
Verification: FULL
Reason: 请求执行、取消与权限边界。
Authorization: 用户已确认总体范围与拆分；本轮只记录，未启动执行。
Delivery: 工作区成果与验收报告；未授权提交、推送、发布。

## Goal and Scope
移除输入模式选择；模型优先；无模型及尚未产生实际操作的模型可用性失败时回退文件名搜索，明确提示。

## Non-goals
不推进其他 Plan，不进行历史迁移、发布或后台框架建设。

## Tasks
执行前按可独立验收的行为细化 Task、候选基线和允许修改范围。

## Acceptance
模型正常、无模型、连接失败有覆盖；取消、权限拒绝、正常无结果不回退；部分执行不重复执行；结果归属原会话。

## Decisions and constraints
遵守 [总索引](2026-09-24-desktop-trial-index.md)、AGENTS.md 和 docs/workflow/README.md。
使用隔离夹具；测试与评审绑定最终候选；未验证项如实记录。

## Evidence
待执行。

## Changes to approved scope
None.
