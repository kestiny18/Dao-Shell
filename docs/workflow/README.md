# Dao-Shell 开发工作流

## 1. 职责

主 Agent 是用户的工作台，负责：

- 澄清目标，记录用户已确认的决策。
- 维护任务范围、验收标准和验证方式。
- 派发 Coder、Tester、Reviewer。
- 核对交接版本、验证证据和角色修改范围。
- 处理失败、歧义和角色之间的分歧。
- 更新任务状态，归档结果并向用户交付。

主 Agent 不把自己参与实现后的检查称为独立验证。
FULL 流程的生产代码修改交给 Coder。
LIGHT 任务可由主 Agent 直接执行。

子 Agent 不自行派生其他 Agent，不推进后续 Task。

## 2. 触发与授权

讨论、方案评审本身不触发实现。

用户要求实现、修复或按已确认方案执行时：
1. 将已确认范围写入任务记录。
2. 选择验证方式并说明理由。
3. 在已授权范围内继续执行，不重复请求同一批准。

用户后续明确决定优先于较早计划。
主 Agent 及时更新 Plan；影响正在执行的工作时，先通知或停止相关角色。

实现过程中发现新增功能需求、关键需求歧义或重大取舍时，
返回主 Agent 处理；不要通过修改验收标准让现有实现自动通过。

“继续”默认推进当前已授权范围内的下一步。
授权范围完成后停止。

## 3. 任务记录

多步骤任务、一般功能开发和关键边界变更使用 docs/plans/ 下的 Plan。

独立、低风险的小修改可在交付记录中直接说明：
目标、范围、验证方式、结果；无需专门创建 Plan。

Plan 是已批准工作范围的持久记录。
代码表示实现状态，测试和评审报告表示特定版本的检查结果。
三者不互相替代。

每个 Task 应对应一个可独立验收的行为或边界。
不要以修改文件数作为拆分任务的唯一标准。

## 4. 验证方式

LIGHT：
- 适用于文档、文案和简单低风险调整。
- 执行并完成必要检查。
- 不要求独立 Tester 或 Reviewer。

STANDARD：
- 适用于一般功能和缺陷修复。
- Coder 实现并自测。
- 至少执行独立 Tester 或 Reviewer 中的一种。
- 主 Agent 记录选择理由；发现风险后可升级 FULL。

FULL：
- 适用于授权、文件写操作、文件身份、确认、取消、持久化、
  凭据处理和 Runtime 等关键边界。
- Coder → Tester → Reviewer 顺序执行。
- 不满足必需验证环节时，不能降级宣称完成。

验证方式在实施前确定。
不得仅为规避失败或节省额度而降低完成要求。

## 5. 派发与上下文

所有角色读取对应角色协议：
- roles/coder.md
- roles/tester.md
- roles/reviewer.md

Tester 和 Reviewer 必须以空白对话上下文启动。
在提供 fork_turns 参数的运行时，显式使用 fork_turns="none"。
其他运行时使用等价的独立上下文机制；
若无法保证，说明限制，不宣称完成了独立验证。

交接包包含：
- Plan 路径、Task 标识和当前目标。
- Scope、Non-goals、验收标准。
- 相关设计决定及理由、长期工程约束。
- 工作目录、比较基线和候选版本。
- 允许修改的测试或夹具范围。
- 验证方式和预期输出。

不提供 Coder 的完整对话、内部推理或正确性自证。
必要的设计理由应来自正式任务和工程文档。

Reviewer 先独立检查需求与改动，再核对 Tester 报告。
后续复验可以提供已有 finding 和复现证据，但不能只检查旧问题。

## 6. 工作区与候选版本

第一版默认顺序执行，共享工作区。
任何时刻只有一个角色修改代码。
角色运行期间，主 Agent 不同时修改被检查的文件。

开始前记录：
- HEAD 和比较基线。
- 已有 staged、unstaged、untracked 改动。
- 哪些改动属于用户或其他任务。
- 当前角色允许修改的范围。

交接时必须能准确识别候选代码：

- 已提交且无额外代码改动时，使用 commit SHA。
- 未提交时，记录 HEAD，并保存 staged / unstaged 的完整差异、
  相关 untracked 文件内容及文件哈希清单。
- 本地快照不得包含凭据或私人数据，不提交到公开仓库。
- 仅记录 HEAD、文件名列表或 git status 不足以识别未提交版本。

主 Agent 可以在当前授权允许时使用提交作为检查点；
不能为了流程方便擅自提交用户未授权的内容。

检查结束后比较完整改动：
- tracked 文件的 staged / unstaged 内容；
- 新增、删除和重命名；
- 相关 untracked 文件。
- 不只比较 git status 或 git diff --name-only。

Git 检查只能发现部分工作区变化，不是权限隔离。
构建输出和临时夹具允许出现在任务指定位置；
不得改写真实用户文件、凭据或外部配置。

发现越界修改：
1. 暂停交接。
2. 保留差异与证据。
3. 由主 Agent 判断恢复或转交 Coder。
4. 不自动 reset、clean 或覆盖已有改动。

## 7. 验证与版本绑定

Coder 先完成自测，再交给独立角色。

Tester 增加测试后，这些测试属于候选版本的一部分。
Reviewer 检查最终生产代码和新增测试。

每份测试和评审结论记录其适用的候选版本。
任何后续代码或测试修改，都由主 Agent 判断影响范围，
并重新执行受影响的测试和评审。

DONE 对应明确的最终候选版本。
如果最终版本与已验证版本不同，必须记录差异及复验依据。
只追加报告、状态等非执行文档时，说明差异即可，
无需机械重复所有测试。

构建产物交付还应记录来源版本和产物校验值。
源代码检查通过不自动证明安装包或干净机器体验通过。

验证命令引用 ../DEVELOPMENT.md 和相关组件文档。
按改动范围选择检查，不为小改动机械执行所有昂贵验证。

## 8. 状态与结果

Task 状态：

PLANNED → CODING → TESTING → REVIEWING → DONE

LIGHT / STANDARD 可以跳过不适用阶段，但记录原因。

额外状态：
- BLOCKED：缺少环境、资源或用户决策，无法继续必需环节。
- CANCELLED：用户取消或任务已明确被替代。

验收项结果：
- PASS：有证据表明满足当前验收标准。
- FAIL：有证据表明违反验收标准。
- NOT_VERIFIED：未执行或证据不足。
- N/A：对本任务不适用，并说明理由。

评审结论：
- NO_BLOCKING_FINDINGS：在已审范围内没有发现阻塞问题。
- CHANGES_REQUIRED：存在需要修复的问题。
- INCOMPLETE：评审范围未完成或必要证据缺失。

没有发现问题不代表证明代码完全正确。

## 9. 失败与分歧

实现缺陷：交给 Coder 修复。
测试错误：交给 Tester 修正测试和结论。
需求歧义：由主 Agent 澄清，必要时询问用户。
环境不足：标记 BLOCKED / NOT_VERIFIED。
范围外改进：记录后续建议，不扩展当前任务。

Coder 可以对 finding 提供反证。
主 Agent 按需求和证据处理，不以角色身份或多数票裁决。
分歧无法解决时，如实返回用户。

同一问题连续两轮修复仍未解决时，停止机械循环，
由主 Agent 重新分析原因和下一步。
资源不足或运行时不支持子 Agent 时，报告限制，
不将自测冒充独立验收。

## 10. 完成条件

主 Agent 核对：

- 当前授权范围内的实现已完成。
- 所有必需验收项有对应证据。
- 当前验证方式要求的独立环节已完成。
- 没有未处理的阻塞问题、角色越界或代码版本不一致。
- 已归档最终版本、验证结果和剩余限制。

如果用户决定缩小范围或接受延期项：
更新 Plan，保留未验证事实；不将未执行检查改写为 PASS。

完成 Task 不自动授权 commit、push、merge 或 release。
只有交付范围明确要求这些动作且已有授权时才执行。

## 11. 归档

主 Agent 维护 Plan，保存角色返回的事实和结论。
不能把部分验证改写为完整通过，也不能抹去未解决的分歧。

默认一个 Task 对应一份 docs/reviews/ 验收记录，
分为测试和评审两部分。
较大任务可拆分报告。

修复过程只保留：
关键 finding、处理结果、最终候选版本和必要复验依据。
不保存完整聊天或重复工具输出。

新主会话从工程规则、当前 Plan、仓库状态和相关报告恢复工作。
不要仅凭旧报告的 DONE 推断当前工作区仍处于同一状态。

## 12. Plan 模板

```markdown
# <任务名称>

Status: PLANNED
Verification: LIGHT / STANDARD / FULL
Reason: <为什么选择此验证方式>
Authorization: <用户已授权的范围>
Delivery: <工作区交付 / commit / push / PR 等，仅记录实际授权>

## Goal
<用户可观察的目标>

## Scope
- ...

## Non-goals
- ...

## Decisions and constraints
- <决定、必要理由和约束>

## Tasks

### T1 — <可独立验收的工作>

Status: PLANNED

Acceptance:
- AC1 [required]: ...
- AC2 [required]: ...

Allowed test changes:
- <具体测试路径、夹具或配置>

Evidence:
- <完成后链接报告>

## Open questions
- ...

## Changes to approved scope
- <后续实际变更；没有则写 None>
```

## 13. 验收记录模板

```markdown
# <任务名称> — Verification

Plan / Task:
Verification mode:
Baseline:
Final candidate:
Environment:

## Test verification

Tested candidate:
Tester context: independent / not independent / not applicable

| Criterion | Result | Evidence |
| --- | --- | --- |
| AC1 | PASS / FAIL / NOT_VERIFIED / N/A | 命令、复现步骤或结果位置 |

Commands and outcomes:
- <命令、结果、必要的输出摘要>

Added tests:
- ...

## Review

Reviewed candidate:
Comparison baseline:
Reviewer context: independent / not independent / not applicable
Conclusion: NO_BLOCKING_FINDINGS / CHANGES_REQUIRED / INCOMPLETE

Findings:
- <ID、严重程度、位置、触发条件、影响、证据>
- <处理结果及复验依据>

## Candidate changes and revalidation
- <测试、评审以后发生了什么变化，如何重新验证>

## Remaining gaps
- <未验证项及是否阻塞当前交付>

## Orchestrator decision

Status:
Reason:
Delivery state:
```
