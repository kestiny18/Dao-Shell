# 开发指南

产品介绍见 [README](../README.zh-CN.md)，初次试用见 [体验指南](GETTING_STARTED.zh-CN.md)。

## 获取与构建

```powershell
git clone https://github.com/kestiny18/Dao-Shell.git
Set-Location Dao-Shell
cargo test --locked --all-targets
cargo run -- --help
```

Rust 版本锁定于 `rust-toolchain.toml`，依赖锁定于 `Cargo.lock`。Windows 推荐已有的 MSVC Rust 与 C++ 构建环境；其他平台目前只做核心编译/逻辑检查，不代表已经交付系统操作适配。

项目名和 Cargo 包名仍是 Dao-Shell / `dao-shell`，用户运行的二进制与命令名是 `daosh`。显式的 `[[bin]]` 配置负责生成 `target/release/daosh.exe`。

缺少 Windows 工具链时，可使用项目的可选 GNU 开发脚本：

```powershell
.\scripts\bootstrap-windows.ps1
.\scripts\dev.ps1 test --locked --all-targets
.\scripts\dev.ps1 run -CargoArgs @('--', '--help')
```

它在用户目录安装 Rust，在 `.tools` 解压经 SHA256 校验的 w64devkit；不改系统 PATH。已有 MSVC Rust 主机配置会沿用。GNU 链接使用 Rust 自带 `rust-lld` 与运行时库。

Unix 构建还需要 C 编译器，以及用于原生 TLS 的 OpenSSL 开发库；SQLite 随依赖编译。

## 验证

```powershell
.\scripts\dev.ps1 fmt --check
.\scripts\dev.ps1 test --locked --all-targets
.\scripts\dev.ps1 clippy -CargoArgs @('--locked', '--all-targets', '--', '-D', 'warnings')
.\scripts\dev.ps1 build --release --locked
.\scripts\smoke-windows.ps1
```

直接用 Cargo 时，对应 `cargo fmt --check`、`cargo test --locked --all-targets`、`cargo clippy --locked --all-targets -- -D warnings`。

测试只修改独立临时目录。Windows 集成用例覆盖真实移动、文件与目录替换、目标冲突、取消、部分完成和崩溃核对。模型协议测试启动本机 HTTP 服务，不需要 API Key，也不代表真实模型已验收。入口检查脚本创建 `.tools/smoke-*` 夹具，测试范围、junction、资源采样与管道确认限制。

CI 分别在 Windows 和 Ubuntu 跑测试；实际运行结果以仓库 Actions 页面为准。首次公开前的本机验证记录见 [实现记录](IMPLEMENTATION.md)。

Windows 进程 CPU 的真实负载复现单独运行（会短暂占用一个测试线程）：

```powershell
.\scripts\dev.ps1 test -CargoArgs @('--locked', '--test', 'resource_sampling', '--', '--ignored', '--nocapture')
```

默认测试覆盖 CPU 时间差计算边界，负载复现不放进常规 CI，避免宿主机调度造成不稳定结果。

## 代码组织

```text
src/
  cli.rs / cli/     # 入口流程、参数定义、终端渲染与本地确认
  dialogue.rs       # 上下文与有限工具调用循环
  model.rs / model/ # Provider 配置、模型传输、固定样例的连接诊断
  settings.rs       # 入口无关的配置保存、版本检查与凭据更新
  runtime.rs        # 执行、权限、持久写操作要求
  runtime/control.rs # 请求互斥、取消、一次性确认
  computer.rs       # 本地电脑概览，带覆盖限制和采样时间
  capabilities.rs   # 六项能力的描述和参数分发
  files.rs          # 范围、查询、对象引用
  operations.rs     # 准备、确认、执行、核验、取消
  storage.rs        # SQLite 操作记录与实例锁
  platform/         # Windows 句柄操作
  resources.rs      # 资源与进程采样
tests/              # 确定性边界与协议测试
scripts/            # 开发、打包和入口检查
docs/               # 体验、设计、实现证据与开发记录
```

本地授权、对象身份、确认和结果状态是代码边界，不能由模型文字决定。修改这些部分时补充有意义的边界测试，不新增任意命令执行或通用任务框架。

## 打包

```powershell
.\scripts\package-windows.ps1
```

需要 PowerShell 7，输出便携目录、ZIP、SHA256 校验文件与依赖许可说明。打包本身不发布 GitHub Release，不代表干净 Windows 环境验收。

`.tools`、`target`、`dist`、数据库、日志、环境文件和 `local` 目录不入库。样例使用虚构文件名；公开错误和截图前移除个人路径、凭据与私人对话地址。
