# Diagnostic only: print the actual elevation bit without account identifiers.
$ErrorActionPreference = 'Stop'
Add-Type -TypeDefinition @'
using System;
using System.Runtime.InteropServices;
public static class DaoShellCiToken {
    [DllImport("advapi32.dll", SetLastError = true)]
    [return: MarshalAs(UnmanagedType.Bool)]
    public static extern bool GetTokenInformation(IntPtr token, int kind,
        out int information, uint length, out uint returned);
}
'@
$identity = [Security.Principal.WindowsIdentity]::GetCurrent()
try {
    $elevation = 0
    [uint32]$returned = 0
    if (-not [DaoShellCiToken]::GetTokenInformation($identity.AccessToken.DangerousGetHandle(), 20, [ref]$elevation, 4, [ref]$returned)) {
        throw [ComponentModel.Win32Exception]::new([Runtime.InteropServices.Marshal]::GetLastWin32Error())
    }
    if ($returned -ne 4) { throw 'Unexpected TOKEN_ELEVATION size' }
    Write-Output "Windows test host TokenElevation: $([bool]$elevation)"
} finally { $identity.Dispose() }
