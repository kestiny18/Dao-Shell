param([string]$Executable = (Join-Path $PSScriptRoot '..\target\debug\examples\inventory_spike.exe'))
$ErrorActionPreference = 'Stop'
$OutputEncoding = [Console]::OutputEncoding = [System.Text.UTF8Encoding]::new($false)
$Executable = [IO.Path]::GetFullPath($Executable)
$workspace = [IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
$experiment = Join-Path $workspace ('.tools\inventory-watch-' + [guid]::NewGuid().ToString('N'))
$root = Join-Path $experiment 'files'
$db = Join-Path $experiment 'index.db'
[IO.Directory]::CreateDirectory((Join-Path $root 'before')) | Out-Null
[IO.File]::WriteAllText((Join-Path $root 'before\keep.txt'), 'keep')
function Query-Hits([string]$Term) {
    $result = & $Executable query --db $db $Term 2>$null
    if ($LASTEXITCODE -ne 0) { return -1 }
    return (($result -join "`n" | ConvertFrom-Json).hits.Count)
}
function Wait-Hits([string]$Term, [int]$Count) {
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        if ((Query-Hits $Term) -eq $Count) { return }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    throw "Query did not converge: $Term -> $Count"
}
function Start-Watch([string]$Name, [int]$ReconcileSeconds = 30) {
    $out = Join-Path $experiment "$Name.jsonl"
    $err = Join-Path $experiment "$Name.err"
    # Start-Process joins arguments into one Windows command line; quote each path explicitly.
    $arguments = @('watch', '--root', ('"' + $root + '"'), '--db', ('"' + $db + '"'), '--seconds', '60', '--reconcile-seconds', [string]$ReconcileSeconds)
    $process = Start-Process -FilePath $Executable -ArgumentList $arguments -WindowStyle Hidden -PassThru -RedirectStandardOutput $out -RedirectStandardError $err
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        if ((Test-Path -LiteralPath $out) -and (Get-Content -LiteralPath $out -Raw) -match 'ready_after_startup_reconcile') { return $process }
        if ($process.HasExited) { throw (Get-Content -LiteralPath $err -Raw) }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    Stop-Process -Id $process.Id -ErrorAction SilentlyContinue
    throw 'Watcher did not become ready'
}
$process = Start-Watch 'first'
try {
    [IO.File]::WriteAllText((Join-Path $root 'added.txt'), 'created')
    Wait-Hits 'added.txt' 1
    [IO.File]::WriteAllText((Join-Path $root 'added.txt'), 'updated file content')
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $result = & $Executable query --db $db 'added.txt'
        if ($LASTEXITCODE -ne 0) { throw 'Query failed during metadata update' }
        $size = ($result -join "`n" | ConvertFrom-Json).hits[0].identity_at_scan.size
        if ($size -eq 20) { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if ($size -ne 20) { throw 'Metadata update did not converge' }
    Remove-Item -LiteralPath (Join-Path $root 'added.txt')
    Wait-Hits 'added.txt' 0
    Rename-Item -LiteralPath (Join-Path $root 'before') -NewName 'after'
    Wait-Hits 'after' 2
    Wait-Hits 'before' 0
    $lines = Get-Content -LiteralPath (Join-Path $experiment 'first.jsonl')
    if (-not ($lines -match 'incremental_snapshot')) { throw 'No incremental file update observed' }
    if (-not ($lines -match 'reconciled_snapshot')) { throw 'No directory reconciliation observed' }
} finally {
    if (-not $process.HasExited) { Stop-Process -Id $process.Id }
    $process.WaitForExit()
}
# Simulate abrupt termination and modifications while the watcher is offline.
[IO.File]::WriteAllText((Join-Path $root 'offline.txt'), 'offline')
Wait-Hits 'offline.txt' 0
$process = Start-Watch 'restart'
try { Wait-Hits 'offline.txt' 1 } finally {
    if (-not $process.HasExited) { Stop-Process -Id $process.Id }
    $process.WaitForExit()
}
$process = Start-Watch 'periodic' 1
try {
    $deadline = [DateTime]::UtcNow.AddSeconds(10)
    do {
        $periodic = (Get-Content -LiteralPath (Join-Path $experiment 'periodic.jsonl') -Raw) -match 'reconciled_snapshot'
        if ($periodic) { break }
        Start-Sleep -Milliseconds 100
    } while ([DateTime]::UtcNow -lt $deadline)
    if (-not $periodic) { throw 'Periodic reconciliation did not run while idle' }
} finally {
    if (-not $process.HasExited) { Stop-Process -Id $process.Id }
    $process.WaitForExit()
}
$report = [ordered]@{
    native_file_create_delete = $true
    native_file_metadata_update = $true
    native_directory_rename_reconcile = $true
    ordinary_file_delta_observed = $true
    offline_change_recovered_at_restart = $true
    periodic_reconcile_without_events = $true
    process_stopped = $true
    note = 'No elevation or USN; forced process termination and startup full scan. Native buffer overflow was not forced.'
}
$report | ConvertTo-Json | Set-Content -LiteralPath (Join-Path $experiment 'report.json') -Encoding UTF8
$report | ConvertTo-Json
Write-Host "实验记录：$experiment"
