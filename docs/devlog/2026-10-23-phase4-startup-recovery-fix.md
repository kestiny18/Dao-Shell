# Startup恢复证明R1修复记录

Date: 2026-10-23
Baseline: `0c00e70fe067f1e7971a8cc810f0ae82e6467752`
Candidate: `81c5ed2f53437f682ca815cd6b95c24de83217fe`
Implementation: READY（仅R1修复范围，整期仍IN_PROGRESS）
Self-test: PASS（R1隔离范围）
Independent testing: PASS（定向隔离范围；实机项未验证）
Independent review: NO_BLOCKING_FINDINGS（R1 CLOSED）

## 修复

禁用和恢复均在移动前可靠记录Startup专用proof：原位置、保留位置、冻结文件身份和SHA256。最终回执丢失后，固定本地startup_targets入口只读核对两端；唯一精确匹配且另一端明确NotFound时记startup_checked_retained/original，保留原动作归因unknown，不改成verified、不自动重放。冲突/替换/内容漂移/错误根/IO错误保持unknown。恢复候选仍以证明匹配当前文件，并要求新的本地确认。其它软件unknown继续阻塞。

checked_retained保留证明不被清理，核对回原位的旧证明可按原清理规则处理。UI中英说明当前位置、历史结果未知与新确认要求。

## Coder自测

- software定向25 PASS；journal定向2 PASS。
- UI原有74 PASS；新增Startup read-only recovery单项PASS（独立Tester将跑完整75项）。
- 严格clippy all-targets PASS；fmt --check和diff --check PASS。
- 覆盖移动后最终写入超预算失败并实际重开、子进程移动后exit(91)不执行Rust清理、restore自身中断、冲突/替换/摘要变动/错误根/两端缺失、前置保存失败0移动、拒绝恢复0移动、软件unknown门禁及保留证明清理。
- 一次新增UI断言误用按钮文案，按实际i18n键修正后通过。未修改生产行为以迎合错误测试。

## 限制

全部文件操作仅临时夹具，无真实Startup/DNS/软件装卸。旧便携测试包仍对应修复前代码，不代表此候选。整期WinGet原生提供程序、实机/UAC/重启及其它未交付能力保持原缺口；不能因R1修复宣布整期READY。独立复验报告另行归档。

## 最终复验

修复候选81c5ed2，独立检查158项PASS，桌面构建PASS；Reviewer确认R1关闭，无新阻塞finding。报告见 docs/reviews/2026-10-23-phase4-startup-recovery-verification.md。仅新增测试和报告归档，不改变被检生产候选。旧便携包不含此修复；第四期其余缺口不改变。
