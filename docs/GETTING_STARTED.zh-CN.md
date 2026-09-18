# 体验 Dao-Shell

[English](GETTING_STARTED.md) · [简体中文](GETTING_STARTED.zh-CN.md)

这是 Windows 命令行开发预览，适合愿意反馈问题的早期使用者。请先用单独的试用目录体验文件整理。

## 获取程序

当前尚未发布正式下载版本。先按 [开发指南](DEVELOPMENT.md) 从源码构建；构建后程序在 `target/release/daosh.exe`，打包目录在 `dist/daosh-0.1.0-windows-x64/`。后续下载入口会放在 [GitHub Releases](https://github.com/kestiny18/Dao-Shell/releases)，并标明验证范围。

下面的示例需要 **PowerShell**，在 `daosh.exe` 所在目录运行。如果当前提示符是 `C:\...>` 而非 `PS C:\...>`，先输入 `powershell`。CMD 不会按 PowerShell 规则处理单引号。

## 第一次启动

推荐先运行交互式设置，不用记住配置参数：

```powershell
.\daosh.exe setup
```

逐项输入读取目录、可写目录、模型的完整接口地址、模型 ID 和密钥环境变量名。目录必须存在；回车保留原范围，输入新目录会替换该类范围，`-` 清空。保存前会展示全部范围，输入 `y` 才保存；`:q` 取消。这里**不输入 API Key 本身**，也不会自动请求模型。更改设置时可再次运行 `setup`。

也可以使用独立命令。创建一个试用目录，再告诉 Dao-Shell 可以访问哪些位置：

```powershell
$demo = Join-Path $env:USERPROFILE 'Documents\DaoShellDemo'
New-Item -ItemType Directory -Force -Path $demo | Out-Null
.\daosh.exe config add-read "$env:USERPROFILE\Downloads"
.\daosh.exe config add-write $demo
```

可读与可写范围分开设置；可写目录同时允许查询。目录必须存在。你也可以用启动参数 `--read-root <目录>`、`--write-root <目录>` 仅为本次运行增加范围，模型不能自行扩大它们。

先不配置模型，也能试一下搜索和资源观测：

```powershell
.\daosh.exe search 合同 --in "$env:USERPROFILE\Downloads" --extension pdf
.\daosh.exe resources --sample-ms 2000 --limit 10
```

`--in` 只授权本次读取，不添加写入权限。第一次体验移动时，请手动把几个不重要的样例文件复制到试用目录。

## 接入模型

当前实现使用支持 function calling 的 Chat Completions 兼容接口。真实服务的兼容性尚未逐一验证，不承诺所有兼容端点都可用。

`--endpoint` 是包含 `/chat/completions` 的完整请求地址，`--model` 使用服务商的实际模型 ID。替换下列占位值：

```powershell
.\daosh.exe config model --endpoint 'https://你的服务地址/v1/chat/completions' --model '实际模型 ID'
$secret = Read-Host 'API Key' -AsSecureString
$env:DAO_SHELL_API_KEY = [Net.NetworkCredential]::new('', $secret).Password.Trim()
.\daosh.exe doctor --check-model
.\daosh.exe
```

OpenAI 官方接口地址为 `https://api.openai.com/v1/chat/completions`；其他服务按其文档填写。本地服务可用 `http://127.0.0.1:<端口>/v1/chat/completions`，无需密钥。远程服务要求 HTTPS。

`Read-Host 'API Key'` 中的文字只是提示，**运行后，在随后出现的输入提示中粘贴密钥**，不要把密钥写进命令。密钥仅在当前 PowerShell 进程及其子进程有效，换终端后要重新设置。程序会去掉首尾空白；内部空白或非法字符会在发请求前明确报错，不显示密钥内容。

配置命令保存前会校验地址和参数。仅填写服务首页、带入 CMD 单引号等情况会给出修正提示；保存失败时保留原配置。

密钥仅从环境变量读取，不写入配置或操作记录。用户输入、候选文件的名称与元数据、资源样本会发往配置的模型服务；首版不读取文件正文、进程命令行或环境变量。会话在退出时结束，不会继续后台工作。

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
| `/results` | 查看当前候选编号；空查询保留上一组并明确提示 |
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

文件搜索只看名称和元数据；扫描达到时间或数量限制会说明覆盖不足。资源观测是短时样本，不能证明完整卡顿根因。打开成功仅表示系统接受了请求，不保证应用已经显示文档。

已有用户在 Windows 上用 DeepSeek 完成首轮真实查找、指代和 PDF 打开；这不代表所有模型兼容，也不代表完整试用验收。15 个固定场景、小批量移动和干净 Windows 环境验证仍待完成。遇到问题，可在 [Issues](https://github.com/kestiny18/Dao-Shell/issues/new/choose) 提供操作步骤和脱敏后的错误信息，不要粘贴 API Key 或个人文件内容。

[回到项目](../README.zh-CN.md) · [当前验证记录](IMPLEMENTATION.md)
