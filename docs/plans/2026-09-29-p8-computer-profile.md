# P8 — 精简电脑档案

Implementation: READY
Self-test: PASS
Independent testing: NOT_RUN
Independent review: NOT_RUN
Authorization: 用户明确执行P7、P8，按顺序实现与自测；独立验证未触发。
Delivery: 工作区原型；不提交、推送或发布。

## Goal / Scope
为用户提供简明、可复制的电脑配置摘要，用于了解设备和向他人描述问题，不堆砌低频参数。

## Prototype requirements
- 折叠详情展示品牌/型号、Windows版本、CPU型号、总内存、显卡型号及磁盘容量；与现有容量展示避免重复。
- 优先使用Windows本机只读系统接口，复用已有采集；默认普通用户权限，读取不足展示未知，不自动提权。
- 多显卡、多磁盘如实区分，注明物理设备或卷的统计口径，不把分区容量重复累加为物理硬盘容量。
- 用户主动点击复制摘要，复制内容与可见信息一致；默认排除序列号、主机名、用户名、IP/MAC、私人路径和凭据。
- 标注系统提供数据与缺失项，不用猜测填补；不依赖联网或模型。
- 低频配置按需加载/缓存并可刷新，不每次资源采样重复枚举硬件。

## Non-goals
硬件跑分、保修查询、驱动更新判断、温度/风扇/SMART、自动上传诊断报告、完整设备管理器。

## Self-test / future validation
实施时用模拟数据验证缺失/多设备、单位、磁盘统计、复制内容及标识信息排除；失败不阻断概览其他区域。兼容性和原生视觉需单独记录，独立验证未触发。

## Open questions
系统接口具体来源和缓存时机在实施前核对本地库及官方文档；不承诺每种设备均提供完整型号。

Baseline: 42d95d44cc8d89a45bd661dd597eecf5bc558396；开始时工作区干净。

## Delivery — 2026-09-30
- Implementation READY / Self-test PASS；Independent testing/review NOT_RUN。
- 默认折叠，展开时经独立窄命令按需加载，本窗口缓存、手动刷新，失败保留上一份摘要和时间。共享请求锁，后台线程采集，不进入模型事实/会话历史或后台轮询。
- 系统来源：sysinfo的CPU/内存/卷容量、Windows BIOS注册表品牌型号、EnumDisplayDevicesW适配器名称。无新依赖/联网/提权。未知及常见OEM占位如实显示，适配器上限64且可能含虚拟显示设备。
- 复制与可见白名单摘要一致，排除主机名、用户名、序列号、IP/MAC、私人挂载路径和凭据。卷编号表示容量，不冒充物理盘总容量。等待剪贴板Promise成功才提示已复制，失败可手动选中复制。
- 修改：新增src/profile.rs及computer_profile命令权限；src/lib.rs、Tauri build/capability/main/bridge、desktop app/home/index/style/tests/README；P7实现保留。
- Coder自测：profile:: 1/1；完整DOM37/37；desktop10/10，补充共享锁定向1/1；resources::3/3、无模型统一入口1/1；核心/桌面fmt、Clippy all-targets -D warnings、JS语法与产品diff检查通过；最终build成功。
- 候选：HEAD42d95d44cc8d89a45bd661dd597eecf5bc558396加P7/P8未提交代码。desktop/src-tauri/target/debug/dao-shell-desktop.exe SHA256 9971F77B83D18317694CA550245D2498D804C8F287E7F81ECA2CD28F7118C5B7，主Agent核对一致。
- 未验证：真实硬件采集、原生视觉、实际剪贴板、普通权限/OEM/虚拟混合显卡兼容性。API核对本地绑定及编译，测试使用模拟数据。无提交推送或其他功能扩展。
