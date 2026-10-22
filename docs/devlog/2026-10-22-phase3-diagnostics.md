# 第三期：短时诊断原型交付

Date: 2026-10-22 (Asia/Shanghai)
Implementation: READY（本机受支持范围的原型）
Self-test: PASS
Independent testing: NOT_RUN
Independent review: NOT_RUN

## 候选与实现

基线 `f7f4df7a139eb28423e72766c7f5a46a89ef4706`。本轮用户明确批准第三期完整实施和小步本地提交；未推送、发布、安装组件或实施第四期维护。

| 提交 | 内容 |
| --- | --- |
| `12fba26` | 第三期授权计划及此前已要求的第四期设计归档；第四期仍PLANNED |
| `e3fd855` | 有界序列、目标端点引擎、本机provider与初始自测 |
| `7ef524f` | 有界投影、稳定启动项分页、固定IP HTTPS与受控TLS验证 |
| `89b2911` | 桌面趋势/对比、目标确认、共享/取消、补充覆盖与协议回归 |
| `678443b` | 最终代码候选：首个有效区间的统计权重修正 |

新增 `src/diagnostics/{mod,sampler,network,coverage}.rs`，复用 observation Store、Runtime、FileSession、Bridge、共享和取消边界；没有新调度平台、常驻daemon或维护执行器。四个新桌面工具为 resource_observe、observation_compare、network_diagnose、startup_list；CLI能力不扩大。

3A：30/60秒、1/2秒间隔，持续System/Networks/PDH基线，不每秒重扫应用目录。只采集选定域；至多61点、5个明确进程实例、8个接口。单调时钟调度，延迟/睡眠空档/计数回退产生缺失；每点500ms软预算，后续慢点可跳过。同步原生调用不承诺硬中断。CPU与速率值按其结束区间加权，首个完整区间不能因基线值unknown而漏计；内存等点值使用相邻有效点的右端矩形积分，未知空档不填补。明确记录有效/缺失数、实际权重时长、峰值位置、显示阈值及超过阈值的时长，不据此认定故障或根因。

会话诊断缓存最多4组、8MiB、10分钟，与原单样本8MiB预算分开；启动项分页也使用该有界诊断缓存。模型只获得本地确定性摘要及最多20代表点，32KiB投影预算优先保留摘要，必要时显式记录省略指标数；原生身份不进入模型。UI保留有界完整序列、缺口趋势和明确两次观测的比较。停止保留完成样本，不再请求模型；模型断线不删除本地观测。显式摘要解释仅一次、无工具请求，模型返回工具调用也不执行。重启只保留可见摘要文本。

3B：本地接口/默认路由/DNS/代理配置读取，不主动探测。主动探测须本地明确主机、端口、范围并勾选确认后登记；登记不发请求，引用3分钟单次使用，模型无登记/URL入口。DNS使用系统resolver；连接前重新核对范围；HTTPS绑定核对后的IP、校验证书和主机名，只做根路径HEAD，不跟随redirect、不带认证/Cookie/自定义请求头。最多3目标、每层实际一次尝试、单层3秒和总墙钟20秒软预算；DNS后台解析并发硬限3，迟到结果不回写已结束请求。

**探测路径是 `direct_no_proxy`**，明确禁用应用代理，不能代表系统代理/VPN路径或全机连通性。公网地址策略保守拒绝私网、loopback、link-local及特殊范围，IPv6覆盖也刻意收窄；局域网/本机目标需独立local确认，已知元数据地址禁止。仅接受ASCII主机名/IPv4与根路径；IPv6字面量、认证、任意路径/query和自动redirect不支持。真实公网、系统代理/VPN行为没有作为本次自测通过项。

3C：I/O直接GetProcessIoCounters并在同handle校验精确creation FILETIME，语义是全部进程I/O，非物理磁盘吞吐。旧观测仅提供选择线索，内部精确身份不出现在普通观测回执；可读性不足不伪造零。GPU使用固定English PDH计数器，至多16个显存计数器与32个独立引擎实例，分开dedicated/shared，不求和为整机GPU%。电源通过GetSystemPowerStatus，unknown、无电池与剩余时间未知分开。

应用清单新增**当前用户包注册关联的应用入口发现**（不推断商店安装渠道），便携应用仅允许本地明确登记既有授权目录内EXE，登记后使app_list静态缓存失效；权限/配置变更重建会话并清除登记。新增Store/便携启动provider未开放，显示launch_unavailable；running视图仍仅原有保守匹配，不猜测新来源运行状态。启动项读取Run/RunOnce与Startup文件夹；登记不证明enabled，值永不执行，命令/路径仅本地显示，不进入模型/工作区文本。固定PowerShell provider只选择代码内脚本，绝对系统路径、NoProfile/非交互、2MiB输出/12秒上限、取消清理自己的进程，无模型脚本入口。

第二期正常关闭继续BLOCKED（close_identity_not_atomic），本期不修改该结论。Store/便携本期必选为发现覆盖，未将未开放的新启动provider称为已完成动作能力。

## 自测与适用版本

- `.tools/phase3/t0-native.json`：生产provider普通用户本机只读实验：自身I/O可读，38个有界GPU计数器，AC在线、电池100%、剩余时间unknown，36个当前用户包关联入口，网络配置与用户启动项可读。原始初探为t0-readonly.json；无提权。
- `.tools/phase3/core-tests-final.log`：全目标回归通过，lib154 passed/3 ignored，协议21、文件会话5等通过；其后只对相关新增边界进行下面的定向验证，没有重复未受影响全集。
- `.tools/phase3/diagnostics-statistics-final.log`：最终候选17/17，覆盖取消保留、计数回退、虚拟时间缺失/睡眠、加权及首区间、预算/过期/跨会话、比较口径/实例、稳定分页、缺电池/未知、I/O精确身份、命令字符串投影、端点伪造/范围与TLS。
- `.tools/phase3/final-protocol.log`：协议22/22、文件会话6/6，包含取消后无第二次模型请求、模型不能授权联网、单次无工具解释、便携登记不能扩大Scope。
- `.tools/phase3/ui-tests-final.log`：72/72；归属、停止后最后摘要/拒绝迟到点、事件去重、目标勾选、命令文本不入历史，中英键和既有交互通过。
- `.tools/phase3/desktop-tests.log`：桌面Rust22/22。core-clippy-final.log、desktop-clippy.log及最终statistics-clippy.log通过；core/desktop fmt和diff空白检查通过。desktop-build-statistics.log为最终Debug构建。
- 本机受控TCP/TLS验证：TCP成功但TLS失败分层；额外在测试客户端仅信任合成证书，以保留production TLS校验的同一HTTPS实现验证 `.invalid` 主机固定IP可达、显式坏代理被禁用、HEAD无Cookie/auth、302至metadata不跟随。测试证书/PFX只用于隔离服务，不安装信任根、不放宽production信任。

真实WebView证据分版本保留：

| 证据 | 适用代码及结果 |
| --- | --- |
| `.tools/phase3-smoke-20261022a/evidence.json` | 首次旧UI构建，11项PASS/6次本机固定响应；不冒充最终UI |
| `.tools/phase3-smoke-20261022b/evidence.json` | `89b2911`，11项PASS/6次本机固定响应，60,042ms/31点正常结束，2,080ms/3点停止；中英单位/趋势/状态、配置、端点/分层、历史与共享通过 |
| `.tools/phase3-statistics-20261022c/evidence.json` | 最终`678443b`新构建，仅验证受影响的原生短时停止序列，确认基线unknown后的首个完整CPU区间计入权重，PASS；完整序列与摘要在该证据中 |

b的完整流程与截图不改写为最终候选整套重跑。最终候选仅统计有变，使用17项确定性检查、c原生验证和最终构建绑定；不重跑不受影响的60秒/网络。

早期回归曾因旧schema白名单期望未更新而失败，修正为16个桌面工具；新增FILETIME最初暴露到本地回执被既有隐私断言发现，改为只保留内部缓存，断言保留并通过。一次UI新增停止测试错误等待未完成Promise，修正测试驱动顺序后72项通过；另一次不存在的测试target命令未执行测试，不计PASS。Clippy样式告警修正后通过。

截图：`.tools/phase3-smoke-20261022b/series-zh.png`、`.tools/phase3-smoke-20261022b/network-en.png`。UI源码在最终统计修正中未变。

## Debug与隔离试用

`E:\WorkSpace\Dao-Shell\desktop\src-tauri\target\debug\dao-shell-desktop.exe`

SHA256：`D846F6E379CE2629A49C1A25F20BCA6EC79AC0BCF3A576075AC19A4241FC1697`

```powershell
.\scripts\desktop.ps1 build
node scripts/diagnostics-smoke.mjs .tools/my-new-phase3-trial desktop/src-tauri/target/debug/dao-shell-desktop.exe
# 仅最终统计路径的短时验证，也必须新目录：
node scripts/diagnostics-smoke.mjs .tools/my-new-statistics-trial desktop/src-tauri/target/debug/dao-shell-desktop.exe --statistics-only
```

完整脚本使用隔离配置/WebView、固定本机模型及localhost TLS服务器；不会调用云API或操作真实办公应用、真实文件、系统设置/服务。正常关闭自己的隔离桌面。只读provider会读取本机配置/程序/启动项，原生详情留在本地证据中，不提交这些证据文件。

未验证：真实云模型理解、其他Windows版本/驱动/机器、无电池物理机、真实公网/代理/VPN差异、真实休眠唤醒长时序与长期运行、实际权限/磁盘/驱动故障矩阵、Release/安装包。相应模拟/纯函数断言不等于这些实机场景通过。独立测试与独立评审均NOT_RUN。

保留且未stage的外部路径：`docs/plans/2026-10-17-p15-release-0.1.0.md`、`docs/plans/2026-10-20-v0.1.0-independent-verification.md`、`docs/reviews/2026-10-20-v0.1.0-independent-verification.md`。
