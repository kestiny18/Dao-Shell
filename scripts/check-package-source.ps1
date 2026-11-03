#Requires -Version 7.0
param([ValidateSet('cli','desktop')][string]$Target='desktop')
$ErrorActionPreference='Stop'
$root=[IO.Path]::GetFullPath((Join-Path $PSScriptRoot '..'))
. (Join-Path $PSScriptRoot 'package-source.ps1')
$receipt=Join-Path $root ".tools/package-check-$Target.json"
if(Test-Path -LiteralPath $receipt){Remove-Item -LiteralPath $receipt}
$before=Get-CheckedSource $root
Invoke-PackageCheck { & "$PSScriptRoot/dev.ps1" fmt -CargoArgs @('--check') }
Invoke-PackageCheck { & "$PSScriptRoot/dev.ps1" test -CargoArgs @('--locked','--all-targets') }
Invoke-PackageCheck { & "$PSScriptRoot/dev.ps1" clippy -CargoArgs @('--locked','--all-targets','--','-D','warnings') }
if($Target -eq 'desktop') {
Invoke-PackageCheck { & "$PSScriptRoot/desktop.ps1" fmt -CargoArgs @('--check') }
Invoke-PackageCheck { & "$PSScriptRoot/desktop.ps1" test }
Invoke-PackageCheck { & "$PSScriptRoot/desktop.ps1" clippy -CargoArgs @('--all-targets','--','-D','warnings') }
Invoke-PackageCheck { & "$PSScriptRoot/desktop.ps1" build }
    & npm ci --prefix (Join-Path $root 'desktop'); if($LASTEXITCODE -ne 0){throw 'npm ci failed'}
    & npm test --prefix (Join-Path $root 'desktop'); if($LASTEXITCODE -ne 0){throw 'UI tests failed'}
}
if($before -ne (Get-CheckedSource $root)){throw 'Source changed during checks; receipt not written'}
@{version=1;target=$Target;result='PASS';fingerprint=$before;checked_at=[DateTime]::UtcNow.ToString('o')} | ConvertTo-Json | Set-Content -LiteralPath $receipt
Write-Host "Successful check receipt: $receipt"
