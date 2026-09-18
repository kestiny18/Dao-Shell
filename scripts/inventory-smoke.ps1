param(
    [ValidateRange(10, 100000)][int]$FileCount = 2000,
    [string]$Executable = (Join-Path $PSScriptRoot '..\target\debug\examples\inventory_spike.exe')
)
$ErrorActionPreference = 'Stop'
$OutputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$experiment = Join-Path $workspace ('.tools\inventory-smoke-' + [guid]::NewGuid().ToString('N'))
$root = Join-Path $experiment 'files'
$db = Join-Path $experiment 'index.db'
New-Item -ItemType Directory -Path $root -Force | Out-Null
function Invoke-Index {
    param([string[]]$Arguments)
    $output = & $Executable @Arguments
    if ($LASTEXITCODE -ne 0) { throw "inventory_spike failed ($LASTEXITCODE)" }
    return ($output -join "`n" | ConvertFrom-Json)
}
for ($i = 0; $i -lt $FileCount; $i++) {
    $directory = Join-Path $root ('四川项目{0:D3}\客户端初始化' -f [int][Math]::Floor($i / 100))
    [IO.Directory]::CreateDirectory($directory) | Out-Null
    [IO.File]::WriteAllText((Join-Path $directory ('dump-client-{0:D6}.sql' -f $i)), 'fixture')
}
$outside = Join-Path $experiment 'outside'
[IO.Directory]::CreateDirectory($outside) | Out-Null
[IO.File]::WriteAllText((Join-Path $outside 'must-not-be-indexed.txt'), 'outside')
New-Item -ItemType Junction -Path (Join-Path $root 'outside-link') -Target $outside | Out-Null
$snapshot = Invoke-Index @('scan', '--root', $root, '--db', $db, '--max-entries', ([string]($FileCount * 2)), '--max-seconds', '120')
if ($snapshot.errors -ne 0 -or $snapshot.skipped_links -ne 1) { throw 'Unexpected scan coverage' }
$excluded = Invoke-Index @('query', '--db', $db, 'must-not-be-indexed')
if ($excluded.hits.Count -ne 0) { throw 'Junction target was indexed' }
$timings = @()
$lateTimings = @()
$missingTimings = @()
for ($i = 0; $i -lt 30; $i++) {
    $query = Invoke-Index @('query', '--db', $db, '客户端', '初始化', '.sql')
    if ($query.hits.Count -ne [Math]::Min(20, $FileCount) -or $query.more -ne ($FileCount -gt 20)) { throw 'Query result mismatch' }
    $timings += $query.query_ms
    $late = Invoke-Index @('query', '--db', $db, ('dump-client-{0:D6}' -f ($FileCount - 1)))
    if ($late.hits.Count -ne 1) { throw 'Rare-match query mismatch' }
    $lateTimings += $late.query_ms
    $missing = Invoke-Index @('query', '--db', $db, 'does-not-exist')
    if ($missing.hits.Count -ne 0) { throw 'Missing query mismatch' }
    $missingTimings += $missing.query_ms
}
$original = Join-Path $root '四川项目000'
$renamed = Join-Path $root '已归档'
Rename-Item -LiteralPath $original -NewName '已归档'
$stale = Invoke-Index @('query', '--db', $db, '四川项目000')
if ($stale.hits.Count -eq 0) { throw 'Expected an explicitly stale snapshot before reconciliation' }
$after = Invoke-Index @('scan', '--root', $root, '--db', $db, '--max-entries', ([string]($FileCount * 2)), '--max-seconds', '120')
$old = Invoke-Index @('query', '--db', $db, '四川项目000')
$new = Invoke-Index @('query', '--db', $db, '已归档', 'dump-client-000000')
if ($old.hits.Count -ne 0 -or $new.hits.Count -ne 1) { throw 'Directory rename reconciliation failed' }
Remove-Item -LiteralPath (Join-Path $renamed '客户端初始化\dump-client-000000.sql')
$afterDelete = Invoke-Index @('scan', '--root', $root, '--db', $db, '--max-entries', ([string]($FileCount * 2)), '--max-seconds', '120')
$deleted = Invoke-Index @('query', '--db', $db, 'dump-client-000000')
if ($deleted.hits.Count -ne 0) { throw 'Deletion reconciliation failed' }
$probe = Invoke-Index @('probe', '--root', $root)
$sorted = @($timings | Sort-Object)
$lateSorted = @($lateTimings | Sort-Object)
$missingSorted = @($missingTimings | Sort-Object)
$report = [ordered]@{
    fixture_files = $FileCount
    snapshot_entries = $snapshot.entries
    initial_scan_ms = $snapshot.scan_ms
    first_page_sql_ms_p50 = $sorted[14]
    first_page_sql_ms_p95 = $sorted[28]
    rare_match_sql_ms_p50 = $lateSorted[14]
    rare_match_sql_ms_p95 = $lateSorted[28]
    no_match_sql_ms_p50 = $missingSorted[14]
    no_match_sql_ms_p95 = $missingSorted[28]
    database_bytes = (Get-Item -LiteralPath $db).Length
    queries_in_separate_processes = 90
    junction_excluded = $true
    rename_and_delete_reconciled = $true
    probe = $probe
    note = 'Synthetic fixture; SQL-only timings exclude process startup and JSON output. OS disk cache is not cleared. No incremental event consumption.'
}
$report | ConvertTo-Json -Depth 10 | Set-Content -LiteralPath (Join-Path $experiment 'report.json') -Encoding UTF8
$report | ConvertTo-Json -Depth 10
Write-Host "实验文件保留在 $experiment"
