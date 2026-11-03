#Requires -Version 7.0
$ErrorActionPreference='Stop'
. "$PSScriptRoot/package-source.ps1"
$fixture=Join-Path ([IO.Path]::GetTempPath()) ('dao-package-fixture-'+[guid]::NewGuid())
New-Item -ItemType Directory -Path (Join-Path $fixture 'src') -Force | Out-Null
try {
 $blocked=$false;try{Get-CheckedSource $fixture | Out-Null}catch{$blocked=$true};if(-not$blocked){throw 'Invalid repository accepted'}
 git -C $fixture init -q
 'fixture' | Set-Content (Join-Path $fixture 'src/new.rs')
 $receipt=Join-Path $fixture 'proof.json'
 $blocked=$false;try {Assert-CheckedSource $fixture 'cli' $receipt | Out-Null}catch{$blocked=$true};if(-not$blocked){throw 'Missing proof accepted'}
 $fingerprint=Get-CheckedSource $fixture
 @{version=1;target='cli';result='PASS';fingerprint=$fingerprint}|ConvertTo-Json|Set-Content $receipt
 Assert-CheckedSource $fixture 'cli' $receipt | Out-Null
 'unrelated' | Set-Content (Join-Path $fixture 'notes.md')
 Assert-CheckedSource $fixture 'cli' $receipt | Out-Null
 '*.ignored.rs' | Set-Content (Join-Path $fixture '.gitignore')
 'first' | Set-Content (Join-Path $fixture 'src/input.ignored.rs')
 $ignoredBefore=Get-CheckedSource $fixture
 'changed' | Add-Content (Join-Path $fixture 'src/input.ignored.rs')
 if($ignoredBefore -eq (Get-CheckedSource $fixture)){throw 'Ignored build input omitted'}
 'changed' | Add-Content (Join-Path $fixture 'src/new.rs')
 $blocked=$false;try{Assert-CheckedSource $fixture 'cli' $receipt | Out-Null}catch{$blocked=$true};if(-not$blocked){throw 'Changed untracked source accepted'}
 New-Item -ItemType Directory -Path (Join-Path $fixture 'scripts') -Force | Out-Null
 New-Item -ItemType Directory -Path (Join-Path $fixture '.tools') -Force | Out-Null
 Copy-Item -LiteralPath "$PSScriptRoot/check-package-source.ps1","$PSScriptRoot/package-source.ps1" -Destination (Join-Path $fixture 'scripts')
 'exit 7' | Set-Content (Join-Path $fixture 'scripts/dev.ps1')
 & (Get-Process -Id $PID).Path -NoProfile -File (Join-Path $fixture 'scripts/check-package-source.ps1') -Target cli 2>$null
 if($LASTEXITCODE -eq 0 -or (Test-Path (Join-Path $fixture '.tools/package-check-cli.json'))){throw 'Failing check produced PASS receipt'}
 Write-Host 'PASS: missing/stale receipt rejected; untracked source covered; unrelated docs allowed; failed check writes no receipt'
} finally {
 $resolved=[IO.Path]::GetFullPath($fixture)
 $tempRoot=[IO.Path]::GetFullPath([IO.Path]::GetTempPath())
 if($resolved.StartsWith($tempRoot,[StringComparison]::OrdinalIgnoreCase) -and (Split-Path $resolved -Leaf).StartsWith('dao-package-fixture-')){Remove-Item -LiteralPath $resolved -Recurse -Force}
}
