# Dao-Shell 开发工作流

## 1. 默认节奏与职责

MVP 默认流程：讨论 → Plan → Coder 实现与自测 → 原型交付。
Tester、Reviewer 保留为用户手工触发的独立能力。

主 Agent 负责目标、范围、任务记录、派发、交接和交付。
一般功能由 Coder 实现；文档、文案及简单低风险修改可由主 Agent 直接完成。
主 Agent 不把自己参与实现后的检查称为独立验证。
子 Agent 不自行派生其他 Agent，不推进后续 Task。

先给用户可试用结果，依据体验反馈迭代；方向稳定后再集中验证。
不为得到“全部通过”而自动增加角色、反复构建或扩展验收范围。

## 2. 触发与授权

讨论和方案评审本身不触发实现。
用户要求实施、修复或按已确认方案执行时，记录范围后继续工作，
不重复请求同一批准。关键需求歧义、新增实质性范围或重大取舍返回用户决定。
用户后续明确决定优先，主 Agent 更新 Plan 并协调正在运行的角色。

独立验证仅由用户明确触发，例如：
- “让 Tester 验证配置保存和旧配置兼容性。”
- “让 Reviewer 检查这次权限相关改动。”
- “对 P1–P3 做一次完整验收。”

用户可以单独选择 Tester 或 Reviewer。
完整验收包含独立 Tester 和 Reviewer，默认顺序执行。
“开始开发”“继续”“做个原型”不自动授权独立验收。
“给朋友试用”“准备发布”也不自动启动角色：说明验证缺口并建议检查范围，
发布行为仍遵循用户实际授权，不额外设置一律适用的审批门槛。

用户已明确要求独立验证时，主 Agent 在该范围内自行协调，不逐次询问。
推荐验证是交付建议，不是默认暂停点；没有需要用户决定的实质问题时，
先完成已授权的原型交付，不等待用户选择是否增加角色。
仅要求测试或评审时，先返回结果；实现修复必须属于已有授权，
不能从“评审”自行扩展为修改生产代码。
已授权修复与验收闭环时，可协调 Coder 修复并复验，无需反复触发。

“继续”推进当前已授权范围内的下一步；范围完成后停止。

## 3. 任务记录与旧计划

多步骤功能和关键边界变更使用 docs/plans/ 下的 Plan。
小修改在交付中简述目标、范围、自检和结果即可，不必单独建 Plan。
每个 Task 对应可试用的行为或边界，不以文件数量拆分。

Plan 记录已批准范围；代码表示实现状态；报告表示特定版本的验证结果。
不要通过放宽验收标准让现有实现自动通过。
区分本轮原型必须达到的要求与之后独立验收需要检查的项目，
未验证事实持续保留，不能把承诺的功能悄悄变成未来事项。

本工作流自本次调整起取代旧 LIGHT / STANDARD / FULL 自动调度规则。
旧 Plan 的 Verification: FULL / STANDARD 仅作为风险和历史检查范围的参考，
不自动派发 Tester / Reviewer。恢复未开始或中断的工作时，
按当前用户指令更新执行方式和状态，不需要重新批准同一开发范围。
用户当前明确要求完整验收时仍执行完整验收。
已完成 Plan 和历史报告保留原状态和证据，不批量重写或降低原结论。

## 4. 自测与试用范围

Coder 必须完成与改动相称的自测：
- 可执行代码检查相关构建、语法或类型，验证主要使用路径。
- 运行受影响的已有测试；明确缺陷修复补必要回归测试。
- 修改授权、文件身份、确认、取消、持久化等边界时补有意义的边界检查。
- 说明命令、结果、已知问题及未验证范围，不以测试数量代替结论。

验证命令引用 [开发指南](../DEVELOPMENT.md) 和相关组件文档。
不机械重复未受影响的全量构建、原生窗口检查和昂贵测试。
新改动、失败或证据不足时再追加相关检查。

界面、布局和只读展示可较早交付体验。
涉及文件写操作、权限、凭据、管理员执行等改动时，
原型先用隔离配置、模拟响应和虚构文件试用，不用真实用户数据做测试。
准备让新行为用于真实数据时，说明具体风险并推荐相关验证，
不因风险标签自动派发角色，也不将未验原型描述为适合稳定使用。
已知会误操作或破坏数据的行为不能作为可用路径交付；修复或隔离该路径。

## 5. 独立角色的交接

触发后读取对应协议：
- [Coder](roles/coder.md)
- [Tester](roles/tester.md)
- [Reviewer](roles/reviewer.md)

Tester、Reviewer 使用空白对话上下文启动；
运行时提供 fork_turns 时显式使用 fork_turns="none"。
无法实现独立上下文时说明限制，不用自测冒充独立验收。

交接包包含：
- Plan / Task 或多个相关任务的明确检查范围。
- 验收标准、必要设计决定及理由、工程约束。
- 工作目录、固定比较基线和准确候选版本。
- 允许修改的测试、夹具或测试配置路径。
- 用户要求的验证类型和预期输出。

不提供 Coder 完整对话、内部推理或正确性自证。
Reviewer 先独立检查需求与改动，再核对已有 Tester 报告；
单独触发评审不要求先补一轮 Tester。
复验可提供已有 finding 和证据，同时检查本轮修复引入的风险。

## 6. 工作区与版本

默认顺序执行、共享工作区，同一时刻只有一个角色修改代码。
角色检查期间，主 Agent 不同时修改被检查的文件。
开始前记录 HEAD、已有 staged / unstaged / untracked 改动和修改范围，
保留用户或其他任务的工作，不自动 reset、clean 或覆盖。

普通原型交付只需记录 HEAD、未提交改动范围、自测结果和限制；
明确结论对应当时工作区，HEAD 本身不代表全部未提交代码。
不默认创建完整快照、全库哈希清单或独立验收报告。

用户触发独立验证后，再固定准确候选版本：
- 已提交且无额外相关改动时使用 commit SHA。
- 未提交时保存本次候选的 staged / unstaged 完整差异、
  相关 untracked 文件内容及哈希清单，覆盖影响验收的依赖和配置。
- 排除构建缓存及无关文件；不得复制凭据或私人数据。
  若必要内容含敏感信息，使用隔离候选并注明覆盖限制，不伪称完整快照。
- 多个角色复用同一候选；相关内容未变化时不重复制作快照。

检查前后比较相关 tracked 内容、暂存改动、新增、删除和重命名，
不只比较 git status 或文件名列表。
Git 检查是事后检测，不是权限隔离；临时夹具和构建输出仅写入指定位置。
越界时暂停交接、保留差异，由主 Agent 处理，不自动回滚用户改动。

需要构建产物交付时，记录来源版本和产物校验值。
构建通过不代表安装包、真实模型或干净机器体验已验证。

## 7. 状态与结论

分别记录实现、自测、独立测试和独立评审，不再用单个 DONE 代表全部通过。

Implementation：
- PLANNED：待实施。
- IN_PROGRESS：实施中。
- READY：当前原型范围已实现，所需自测通过，可按说明试用。
- BLOCKED：当前交付的必需工作无法完成。
- CANCELLED：已取消或被替代。

Self-test：PASS / FAIL / NOT_VERIFIED / N/A。
Independent testing：NOT_RUN / IN_PROGRESS / PASS / FAIL / INCOMPLETE。
Independent review：NOT_RUN / IN_PROGRESS / NO_BLOCKING_FINDINGS /
CHANGES_REQUIRED / INCOMPLETE。

原型可以是 READY + Self-test PASS + 两项独立验证 NOT_RUN。
交付时明确“原型可试用，独立测试、评审未执行”，然后结束当前实施任务。
需要真实数据、发布或更广范围验证的缺口继续保留。

每个验收项记录 PASS / FAIL / NOT_VERIFIED / N/A 及证据。
未执行不等于通过，没有发现阻塞问题不代表证明全部正确。
独立验证发现阻塞当前原型使用的问题时，把 Implementation 改回
IN_PROGRESS 或 BLOCKED；非阻塞建议不自动阻塞原型交付。

验证状态只对注明的范围和版本有效。
之后继续修改时保留旧报告；受影响的当前独立结论标记 INCOMPLETE（待复验），
未受影响的结论可说明依据后沿用，不能把旧 PASS 当作新版本已验证。
原型开发期间不因此自动启动独立角色；已授权验收闭环内执行受影响的复验。
仅报告、状态等非执行文档变化不机械重跑测试。

## 8. 失败与分歧

实现缺陷交给 Coder；测试错误交给 Tester；需求歧义由主 Agent 澄清；
环境不足记录 NOT_VERIFIED / INCOMPLETE，阻塞当前交付时用 BLOCKED。
范围外改进记录后续建议，不扩展当前任务。
Coder 可以提供反证，主 Agent 按需求和证据处理，不用角色身份或多数票裁决。
同一问题连续两轮修复仍未解决时重新分析原因，不机械循环。
资源不足或独立角色不可用时如实报告，不以自测代替独立结论。

## 9. 交付与归档

原型交付核对：范围已实现、必要自测通过、没有阻塞试用的问题或未处理越界，
并说明试用方法、自测、已知问题、未验证范围和推荐的下一步检查。
简短结果写回现有 Plan 或本轮交付即可，不另建验收报告。
主 Agent 复用 Coder 提供的可核实自测证据；没有新改动、失败或证据疑点时，
不再自行重复同一套构建、测试和原生交互，也不提前准备未触发角色的夹具。

独立验证交付核对：用户要求的检查范围完成、候选一致、证据与结论对应。
可交付 FAIL / CHANGES_REQUIRED 报告，但不能宣称验收通过。
完整验收通过要求必需验收项有证据、Tester PASS、Reviewer 无阻塞发现，
且修复后受影响检查已复验。明确失败、未验证及延期事项，不改变事实。

独立验证默认在 docs/reviews/ 保存一份报告，可覆盖一组相关 Task；
只触发一个角色时，另一角色标记 NOT_RUN。
主 Agent 归档关键 finding、处置、最终版本和复验依据，不保存完整聊天。
建议在能力方向稳定、准备长期使用或对外交付时安排集中验证，
不要让关键能力一直积累到整个 MVP 完成才检查。

新会话从当前 Plan、仓库状态和相关证据恢复；历史 DONE 不证明当前版本。
完成任何阶段都不自动授权 commit、push、merge 或 release。
只执行当前用户实际授权的交付动作。

## 10. Plan 模板

```markdown
# <任务名称>

Implementation: PLANNED
Self-test: NOT_VERIFIED
Independent testing: NOT_RUN
Independent review: NOT_RUN
Authorization: <当前授权范围；独立验证未要求时写未触发>
Delivery: <原型工作区 / 其他实际授权交付>

## Goal / Scope
- ...

## Non-goals / Decisions / Constraints
- ...

## Tasks

### T1 — <可试用行为或边界>
Implementation: PLANNED
Prototype requirements:
- AC1: <本轮必须达到的行为>
Self-test:
- <相关检查>
Deferred independent verification:
- <后续推荐检查，不自动执行>
Trial scope:
- <隔离配置、虚构文件或其他明确试用范围>
Evidence:
- <自测结果；独立验证触发后再链接报告>

## Known issues / Unverified / Open questions
- ...

## Changes to approved scope
- None
```

## 11. 独立验证报告模板（触发后使用）

```markdown
# <任务或任务组> — Verification

Plan / Tasks / Scope:
Requested checks: testing / review / both
Baseline:
Candidate:
Environment:
Independent testing: NOT_RUN
Independent review: NOT_RUN

## Test verification（未触发则写 NOT_RUN）
Tested candidate:
Tester context: independent / not independent
Criteria / results / evidence:
Commands and outcomes:
Added tests:

## Review（未触发则写 NOT_RUN）
Reviewed candidate:
Reviewer context: independent / not independent
Findings: <ID、位置、触发条件、影响、证据及处置>
Conclusion:

## Changes and revalidation
- ...

## Remaining gaps
- <是否阻塞当前交付>

## Orchestrator decision
Implementation:
Verification outcome:
Delivery state:
```
