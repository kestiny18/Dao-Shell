#Requires -Version 7.0
function Get-CheckedSource([string]$Root) {
    $listed = @(& git -C $Root ls-files --cached --others --exclude-standard)
    if($LASTEXITCODE -ne 0){throw 'Cannot enumerate source repository'}
    $files = @($listed | Where-Object { $_ -match '^(src/|tests/|scripts/|\.cargo/|\.github/|\.gitignore$|\.gitattributes$|Cargo\.|rust-toolchain|desktop/(ui/|tests/|src-tauri/|package))' -and $_ -notmatch '(^|/)(target|node_modules)/' })
    # Git exclusions must not hide a real module/include/build input from evidence.
    foreach($directory in @('src','tests','scripts','.cargo','.github','desktop/ui','desktop/tests','desktop/src-tauri/src','desktop/src-tauri/permissions','desktop/src-tauri/capabilities','desktop/src-tauri/icons')) {
        $path=Join-Path $Root $directory
        if(Test-Path -LiteralPath $path -PathType Container) {
            $files += @(Get-ChildItem -LiteralPath $path -File -Recurse | Where-Object { $_.FullName -notmatch '[\\/](target|node_modules|\.tools)[\\/]' } | ForEach-Object { [IO.Path]::GetRelativePath($Root,$_.FullName).Replace('\','/') })
        }
    }
    if($files.Count -eq 0){throw 'Source inventory is empty'}
    $entries = @($files | Sort-Object -Unique | ForEach-Object { $p=Join-Path $Root $_; if(Test-Path -LiteralPath $p -PathType Leaf) { "$_`t$((Get-FileHash -LiteralPath $p -Algorithm SHA256).Hash)" } else { "$_`tDELETED" } })
    $hash=[Convert]::ToHexString([Security.Cryptography.SHA256]::HashData([Text.Encoding]::UTF8.GetBytes(($entries -join "`n"))))
    return $hash
}
function Assert-CheckedSource([string]$Root,[string]$Target,[string]$Receipt) {
    if(-not(Test-Path -LiteralPath $Receipt)){throw 'Missing successful check receipt. Run scripts/check-package-source.ps1 first.'}
    $proof=Get-Content -LiteralPath $Receipt -Raw | ConvertFrom-Json
    if($proof.version -ne 1 -or $proof.target -ne $Target -or $proof.result -ne 'PASS' -or $proof.fingerprint -ne (Get-CheckedSource $Root)){throw 'Source differs from successful checks; rerun scripts/check-package-source.ps1.'}
    return $proof.fingerprint
}
function Invoke-PackageCheck([scriptblock]$Check) {
    $global:LASTEXITCODE=0
    & $Check
    if($LASTEXITCODE -ne 0){throw "Required package check failed ($LASTEXITCODE); no receipt written"}
}
