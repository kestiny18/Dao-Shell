# 第四期独立测试与代码评审

Date: 2026-10-23
Authorization: 用户明确要求“先进行测试和代码review”。仅验证与报告，不修改生产行为、不执行真实系统写操作、不推送或发布。
Candidate: `7884984f1c4a2cfba9a22cdc1a223d7d05a63bae`
Baseline: `5bf53f8f84b9071ba15a485162152d216b782f31`
Implementation: IN_PROGRESS
Independent testing: INCOMPLETE（执行项全通过，必要实机项未验证）
Independent review: CHANGES_REQUIRED（R1，Startup崩溃恢复证明缺失）

## 范围与方式

- 以第四期方案为标准，测试维护身份/来源/协议、本地确认与取消、持久化和unknown恢复、Startup身份移动/恢复、固定DNS调用和桌面/模型边界；核对已知全量回归失败。
- Tester、Reviewer使用空白上下文，同一候选顺序检查；主Agent不同时修改被检代码。
- 只用模拟provider、独立临时夹具和隔离配置；另一台机器由用户操作，真实装卸、Startup和DNS写入维持NOT_VERIFIED。
- 保留已有无关发布计划及验收文档改动。初始差异保存在 `.tools/phase4-independent-20261023/`。
- 输出逐项结果、可复现finding、证据与限制，正式报告归档于 `docs/reviews/2026-10-23-phase4-independent-verification.md`。只要求测试评审，本轮先返回结果，不自动修改生产代码。

## 结果

本地独立测试core271 PASS/4 ignored、desktop Rust24 PASS、UI74 PASS、WebView8 PASS。完整回归旧失败本轮未复现，不等于已修复。Reviewer确认1项P1阻塞问题R1；报告见 docs/reviews/2026-10-23-phase4-independent-verification.md。候选源码未修改，本轮先返回测试与评审结果。
