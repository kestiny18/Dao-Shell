# 体验 Dao-Shell

[English](GETTING_STARTED.md) · [简体中文](GETTING_STARTED.zh-CN.md)

这是 Windows 命令行开发预览，适合愿意反馈问题的早期使用者。请先用单独的试用目录体验文件整理。

## 获取程序

当前尚未发布正式下载版本。先按 [开发指南](DEVELOPMENT.md) 从源码构建；构建后程序在 `target/release/daosh.exe`，打包目录在 `dist/daosh-0.1.0-windows-x64/`。后续下载入口会放在 [GitHub Releases](https://github.com/kestiny18/Dao-Shell/releases)，并标明验证范围。

下面的示例需要 **PowerShell**，在 `daosh.exe` 所在目录运行。如果当前提示符是 `C:\...>` 而非 `PS C:\...>`，先输入 `powershell`。CMD 不会按 PowerShell 规则处理单引号。

## 第一次启动

直接运行程序，未配置模型或缺少可用密钥时，会自动进入终端向导：

```powershell
.\daosh.exe
```

1. 选择 DeepSeek、其他兼容服务或本地模型。DeepSeek 预填地址与 `deepseek-flash`，模型名称可修改；其他服务输入完整接口或服务的 `/v1` 地址，向导会补全 `/chat/completions`。
2. 粘贴 API Key，输入不会回显。无需填写环境变量名。已有密钥时可直接回车复用；本地无密钥服务可跳过此项。
3. 选择是否验证连接。验证最多发送两次固定虚构样例，可能产生服务费用，不读取本机文件。失败后可原地修改密钥、地址或模型，或重试。
4. 新输入的密钥可选“仅本次使用”或“安全保存到 Windows 凭据管理器”。默认仅本次使用；安全保存后，下次启动可直接使用。
5. 选择下载、文档或自定义查找目录。向导不增加写入权限；已有可写范围会明确展示并保留，不可用的旧范围可清空。
6. 核对设置并输入 `y` 保存，直接进入 `dao >` 对话，无需退出重启。

输入 `:q` 或 Ctrl+C 后 Enter 可取消，保存前不修改原配置或已保存密钥。目录输入错误可重新选择。可以跳过模型或连接验证，但跳过不代表模型已经可用。没有查找目录时仍可观测资源。

更改设置时运行：

```powershell
.\daosh.exe setup
```

Windows 保存的密钥属于当前用户、当前电脑，并绑定到完整接口地址及环境变量名；改变接口后不会自动复用原接口保存的密钥。密钥不写入 JSON 配置或操作记录。读取顺序是本次向导输入、已有环境变量、Windows 凭据。选择“仅本次使用”不会删除之前保存的密钥。可在 Windows 凭据管理器的“Windows 凭据 → 普通凭据”中删除 `Dao-Shell/model/` 条目；删除配置文件不会删除凭据。

如果凭据保存失败，程序会明确说明密钥仅本次有效，并继续当前会话。其他平台暂不支持凭据持久保存，可用本次输入或环境变量。

## 手动配置与文件整理

保留独立命令供脚本和高级配置使用。创建试用目录并授权：

```powershell
$demo = Join-Path $env:USERPROFILE 'Documents\DaoShellDemo'
New-Item -ItemType Directory -Force -Path $demo | Out-Null
.\daosh.exe config add-read "$env:USERPROFILE\Downloads"
.\daosh.exe config add-write $demo
```

可写目录同时可读。移动的来源和目的地都必须在可写范围内，每次移动仍需确认。首次移动前，手动复制几个不重要的样例文件到试用目录。

无模型时也能使用：

```powershell
.\daosh.exe search 合同 --in "$env:USERPROFILE\Downloads" --extension pdf
.\daosh.exe resources --sample-ms 2000 --limit 10
```

`--in` 仅授权本次读取，不增加写入权限。启动参数 `--read-root`、`--write-root` 仅为本次运行增加范围。

模型目前使用支持工具调用的 Chat Completions 兼容接口；不同服务仍需分别验证。远程服务要求 HTTPS 和密钥，本地回环服务允许 HTTP。已有环境变量方式继续可用：

```powershell
.\daosh.exe config model --endpoint 'https://你的服务地址/v1/chat/completions' --model '实际模型 ID'
$secret = Read-Host 'API Key' -AsSecureString
$env:DAO_SHELL_API_KEY = [Net.NetworkCredential]::new('', $secret).Password.Trim()
.\daosh.exe doctor --check-model
.\daosh.exe
```

以上密钥环境变量仅在当前 PowerShell 及其子进程有效。普通对话会把输入、候选文件名称与元数据、资源样本发给模型服务；不读取文件正文、进程命令行或进程环境。退出后不继续后台工作。

## 开始交谈

```text
dao > 找试用目录中的合同文件。
dao > 打开第二个。
dao > 把第一个移到试用目录的归档文件夹，先给我看方案。
dao > 查看当前资源压力，解释可能原因。
```

打开和移动会在本地展示具体对象。移动方案包含来源、目的地和将创建的目录，输入 `y` 才执行。用户直接使用 `/open 2` 时，表示已经选择打开最近结果的第二项。

移动暂限同卷的 1–20 个普通文件；不覆盖已有目标，不移动目录、不跟随链接。不支持的动作会报错。取消只停止后续步骤，不撤回已经完成的修改。

## 常用入口

| 输入 | 作用 |
| --- | --- |
| `/search 合同` | 不请求模型，直接搜索 |
| `/results` | 查看当前候选编号；新搜索会替换编号，零结果或失败时清空 |
| `/open 2` | 打开当前候选的第二项 |
| `/move 1,2 C:\完整目的目录` | 展示移动方案，等待本地确认 |
| `/resources` | 采样当前资源 |
| `/history` | 查看本地操作回执 |
| `/reset` | 清除当前对话与候选；保留操作记录 |
| `/quit`、`quit`、`exit`、`退出` | 退出 |

模型连接失败后仍可使用这些入口；不会把自然语言请求悄悄降级成关键词搜索。`Ctrl+C` 请求停止本轮；在等待输入或确认时，再按 Enter 返回。

独立命令还包括：

```powershell
.\daosh.exe doctor
.\daosh.exe config show
.\daosh.exe history --reconcile
.\daosh.exe history --clear
.\daosh.exe search --help
```

`config`、`setup` 和 `doctor` 是外部终端命令，不是在 `dao >` 中输入的对话。误输时会提示退出后执行，不转发给模型。

`doctor` 默认不发送模型请求，逐项检查目录与模型本地配置。**`doctor --check-model` 才会发送最多两次模型请求，可能产生费用**：只使用固定虚构样例，检查工具调用和结果回传，不读取本机文件、目录内容、资源指标或操作记录。成功时显示总耗时和服务报告的 token 数（缺失时显示“未提供”），不代表真实文件场景已验收。错误会区分密钥字符、HTTP 鉴权/权限/地址/限流/服务故障、连接失败或超时；连接错误不凭猜测进一步归因为 DNS 或 TLS。

操作记录位于 `%LOCALAPPDATA%\Dao-Shell`，已完成记录保留 7 天，待核对记录不会自动清除；重启只核对已知对象，不重做上次操作。`history --clear` 需本地确认。

## 已知限制与反馈

自然语言搜索支持多个关键词匹配相对路径，并结合文件类型筛选；直接 `/search` 仍按文件名关键词查询。搜索不读取文件正文，不能仅据文件名确认用途；扫描达到时间或数量限制会说明覆盖不足。资源观测是短时样本，不能证明完整卡顿根因。打开成功仅表示系统接受了请求，不保证应用已经显示文档。

已有用户在 Windows 上用 DeepSeek 完成首轮真实查找、指代和 PDF 打开；这不代表所有模型兼容，也不代表完整试用验收。15 个固定场景、小批量移动和干净 Windows 环境验证仍待完成。遇到问题，可在 [Issues](https://github.com/kestiny18/Dao-Shell/issues/new/choose) 提供操作步骤和脱敏后的错误信息，不要粘贴 API Key 或个人文件内容。

[回到项目](../README.zh-CN.md) · [当前验证记录](IMPLEMENTATION.md)
