# P6 — 电脑信息探索（讨论任务）

Status: DISCUSSION_COMPLETE
Task type: Research / discussion only
Authorization: 用户明确P6不是开发任务；已完成初步探索并选定两个方向加入后续计划。
Delivery: 信息可行性与优先级建议，不开发整张功能清单。
Independent testing: NOT_RUN
Independent review: NOT_RUN

## Findings and decision
- 优先从用户问题出发，而不是堆硬件参数。已有资源采样为“当前为什么卡”提供基础。
- 用户选择“资源压力详情”和“精简电脑档案”作为接下来计划，分别记录为[P7](2026-09-29-p7-resource-pressure.md)、[P8](2026-09-29-p8-computer-profile.md)。当前只登记，均为PLANNED。
- 目录空间探索、网络详情、电池、启动项/安全摘要作为未选建议保留，不从本讨论推导实施授权。
- 温度/风扇/完整SMART、驱动更新判断、跑分、浏览历史及自动修复暂不优先。

## Evidence and limits
初步核对当前src/resources.rs、src/computer.rs及Windows官方接口文档；未读取新用户系统数据，未做本机可用性实验，不声称接口在所有设备上可用。
- [系统型号及内存基础信息](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-computersystem)
- [网络接口信息](https://learn.microsoft.com/en-us/windows/win32/api/iptypes/ns-iptypes-ip_adapter_addresses_lh)
- [Wi-Fi位置访问限制](https://learn.microsoft.com/en-us/windows/win32/nativewifi/wi-fi-access-location-changes)
- [启动项覆盖及权限](https://learn.microsoft.com/en-us/windows/win32/cimwin32prov/win32-startupcommand)
- [电池容量与未知字段](https://learn.microsoft.com/en-us/windows/win32/power/battery-information-str)
- [安全中心查询失败语义](https://learn.microsoft.com/en-us/windows/win32/api/wscapi/nf-wscapi-wscgetsecurityproviderhealth)

## Scope correction
本文件此前沿用开发计划模板和顺序实施表述，现按用户最新明确指令改为讨论任务；不创建定时任务或启动开发。
