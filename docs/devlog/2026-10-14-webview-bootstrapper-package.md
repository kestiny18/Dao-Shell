# Windows 小型 WebView2 引导程序试用包

Implementation: READY（仅小型引导程序打包与构建检查）
Self-test: PASS（构建/静态安装脚本/产物校验；未实际安装）
Independent testing: NOT_RUN
Independent review: NOT_RUN
Authorization: 用户认可内置约2MB引导程序、缺少运行环境时按需下载，要求“打包一个试试”；只构建，不安装、不管理服务、不提交推送。
Baseline: ca051de6cb9517b6285a4dbeed33206786c0e956 + 已有未提交P9/P10/P11与功能完整性修复。

范围：将桌面 `webviewInstallMode` 从 `offlineInstaller` 改为 `embedBootstrapper`，同步随包说明和README；保留旧离线安装包，构建新包并比较实际大小。保留GNU必需的WebView2Loader.dll和固定索引worker。已有运行环境时复用，缺少时联网下载安装完整运行环境。

验证：配置可解析、正式打包脚本成功；检查生成NSIS只嵌入小型bootstrapper、原有运行环境检测和下载/安装失败处理仍存在；新包SHA、x64 GUI与loader/payload资源；不启动安装器或触发UAC。不改模型、查询算法、服务运行状态、源文件或历史索引。

原离线安装包：224145432字节，SHA256 `B18CC65D14B0BB599A7C9432DA54BB0BCAD29D0F3A21391732E9FBA71FAB84AC`。实际内嵌完整WebView2安装器214090448字节。后续追加新包结果；构建通过不代表缺少WebView2/断网/干净机器安装已验证。

## 实际打包结果

正式 `scripts/package-desktop-windows.ps1` PASS；固定worker沿用F588，Release桌面编译1m33s，NSIS构建成功。本轮只是安装依赖分发策略变化，没有修改查询或服务核心。

|产物|大小（字节）|SHA256|
|---|---:|---|
|新 `dist/Dao-Shell-0.1.0-windows-x64-setup.exe`|9026733（约9.03MB /8.61MiB）|`95805BF27541F205B36120061AB0D91A09273B717E0AC1FD7E0C07C57ED0D83F`|
|保留的 `dist/Dao-Shell-0.1.0-windows-x64-offline-setup.exe`|224145432|`B18CC65D14B0BB599A7C9432DA54BB0BCAD29D0F3A21391732E9FBA71FAB84AC`|
|新Release桌面|35582976|`321061476072E877E0BC21B9AD1C833FBC87E626DEF7E116BF0422A5503C7485`|

安装包体积减少95.97%。内置bootstrapper实际1860304字节（约1.86MB），不再嵌入214MB完整离线安装器。旧离线包及其以新文件名生成的SHA校验文件已保存，原F3冻结候选也保留。

`.tools/webview-bootstrapper-20261014/verify-package.py` 全部检查PASS：

- 生成NSIS选择embedBootstrapper，bootstrapper路径有效，完整离线安装器路径为空；已有运行环境HKLM/HKCU检测和安装失败Abort路径仍存在。这是脚本静态检查，不是缺失运行环境实测。
- 随包指南与源码一致，GNU WebView2Loader.dll仍附带（160320字节）。
- Release为x64/GUI，内嵌worker完整字节和预期SHA与F588一致。
- 新包SHA文件与实际字节一致；旧离线包校验仍为B18C，没有覆盖丢失。新产物、NSIS脚本和校验结果冻结至上述.tools目录，构建日志为build.log，结果为artifacts.json。
- `git diff --check` PASS（仅原有换行提示）。没有改动原有P9/P10/P11功能代码、安装/停止/升级实际服务、触发UAC、提交或推送。

试用入口：双击新的小型安装包即可由安装器检查运行环境；已有WebView2时复用，缺少时需要网络下载完整运行环境。本轮没有执行安装包；缺少WebView2、断网、干净机器与实际NSIS安装仍NOT_VERIFIED。P11服务部署/恢复等原有未验项不因本次打包关闭，独立验证NOT_RUN。
