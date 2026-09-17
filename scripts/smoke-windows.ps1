param([string]$Binary)
$ErrorActionPreference = 'Stop'
$workspace = [System.IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
if (-not $Binary) { $Binary = Join-Path $workspace 'target\release\daosh.exe' }
$Binary = (Resolve-Path -LiteralPath $Binary).Path
$fixture = Join-Path $workspace ('.tools\smoke-' + [guid]::NewGuid().ToString('N'))
$inside = Join-Path $fixture 'allowed'
$outside = Join-Path $fixture 'outside'
New-Item -ItemType Directory -Path $inside,$outside | Out-Null
Set-Content -LiteralPath (Join-Path $inside 'contract.txt') -Value 'Fixture document' -Encoding UTF8
Set-Content -LiteralPath (Join-Path $outside 'secret.txt') -Value 'Outside the allowed scope' -Encoding UTF8
New-Item -ItemType Junction -Path (Join-Path $inside 'redirect') -Target $outside | Out-Null
$config = Join-Path $fixture 'config.json'
& $Binary --config $config config add-read $inside
if ($LASTEXITCODE -ne 0) { throw 'Config setup failed' }
$result = & $Binary --config $config search contract --extension txt --json | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or $result.items.Count -ne 1 -or $result.skipped -lt 1) { throw 'Scoped search or junction exclusion failed' }
$noLeak = & $Binary --config $config search secret --json | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or $noLeak.items.Count -ne 0) { throw 'Scope escaped through junction' }
$sample = & $Binary --config $config resources --sample-ms 500 --limit 3 --json | ConvertFrom-Json
if ($LASTEXITCODE -ne 0 -or $sample.sample_ms -lt 500 -or $sample.processes.Count -gt 3) { throw 'Resource sample failed' }
# Redirected input must not turn the following 'y' into local approval.
$chat = @('/search contract', "/move 1 $inside\archive", 'y', '/quit') | & $Binary --config $config --data-dir (Join-Path $fixture 'state') --write-root $inside
if ($LASTEXITCODE -ne 0 -or (Test-Path -LiteralPath (Join-Path $inside 'archive'))) { throw 'Non-interactive mutation was not rejected' }
[pscustomobject]@{ Binary=$Binary; ScopedSearch=$true; JunctionExcluded=$true; PipedApprovalRejected=$true; ResourceSampleMs=$sample.sample_ms; ProcessCount=$sample.processes.Count; Fixture=$fixture } | ConvertTo-Json
