# The steps of Kiri on Windows that need an administrator, run elevated by
# kiri-win (crates/kiri-win/src/setup.rs and dns.rs): Windows asks for an
# administrator's approval first. Installed beside kiri-win.exe, which checks
# it against the SHA-256 it was built with and runs
#   powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File kiri-admin.ps1 -Wsl -Msi <wsl .msi> -Sha256 <hex>
#   powershell -NoProfile -NonInteractive -ExecutionPolicy Bypass -File kiri-admin.ps1 -Dns -Suffix kiri.local [-Off]
# A script file with parameters, not script text in kiri-win.exe handed to
# PowerShell encoded: an elevated, hidden PowerShell fed an encoded script
# is what malware droppers do, and Defender's heuristics quarantined a
# kiri-win.exe that carried that (as update.rs says of kiri.ps1 -Update).
param(
  [switch]$Wsl,
  [string]$Msi,
  [string]$Sha256,
  [switch]$Dns,
  [ValidatePattern('^[A-Za-z0-9.-]+$')]
  [string]$Suffix = "kiri.local",
  [switch]$Off
)
$ErrorActionPreference = "Stop"

# -Wsl: setup step 1, in one elevated run: Virtual Machine Platform on, and
# WSL installed from the MSI kiri-win downloaded. That MSI lives in a
# user-writable folder, so a process at the user's own integrity could swap
# it between kiri-win's SHA-256 check and this run: it is checked again
# here, as admin, on a copy the user cannot touch, and must be signed by
# Microsoft. setup.rs reads the exit code: 22 the copy does not match
# -Sha256, 23 not validly signed by Microsoft, 20 Virtual Machine Platform
# failed, 21 the WSL installer failed, 3010 done but Windows must restart.
if ($Wsl) {
  if (-not $Msi -or -not $Sha256) { throw "-Wsl needs -Msi and -Sha256" }
  $dst = Join-Path $env:windir ('Temp\kiri-wsl-' + [guid]::NewGuid().ToString('N') + '.msi')
  Copy-Item -LiteralPath $Msi -Destination $dst -Force
  if ((Get-FileHash -Algorithm SHA256 -LiteralPath $dst).Hash.ToLower() -ne $Sha256.ToLower()) { Remove-Item -LiteralPath $dst -Force; exit 22 }
  $sig = Get-AuthenticodeSignature -LiteralPath $dst
  if ($sig.Status -ne 'Valid' -or $sig.SignerCertificate.Subject -notlike '*Microsoft*') { Remove-Item -LiteralPath $dst -Force; exit 23 }
  $vmp = (Start-Process "$env:SystemRoot\System32\dism.exe" -ArgumentList '/online', '/enable-feature', '/featurename:VirtualMachinePlatform', '/all', '/norestart' -Wait -PassThru -WindowStyle Hidden).ExitCode
  if ($vmp -ne 0 -and $vmp -ne 3010) { Remove-Item -LiteralPath $dst -Force; exit 20 }
  $msiexec = (Start-Process "$env:SystemRoot\System32\msiexec.exe" -ArgumentList '/i', ('"' + $dst + '"'), '/qn', '/norestart' -Wait -PassThru -WindowStyle Hidden).ExitCode
  Remove-Item -LiteralPath $dst -Force
  if ($msiexec -ne 0 -and $msiexec -ne 3010) { exit 21 }
  if ($vmp -eq 3010 -or $msiexec -eq 3010) { exit 3010 } else { exit 0 }
}

# -Dns: the name-resolution policy (NRPT) rule that sends `.<suffix>`
# lookups to kiri-win's DNS answerer on 127.0.0.1 (dns.rs), added, or
# removed with -Off. Any rule already there for the suffix goes first, so
# adding twice leaves one. A failure exits 1, which dns.rs reports.
if ($Dns) {
  $namespace = ".$Suffix"
  Get-DnsClientNrptRule | Where-Object { $_.Namespace -contains $namespace } | Remove-DnsClientNrptRule -Force
  if (-not $Off) {
    Add-DnsClientNrptRule -Namespace $namespace -NameServers '127.0.0.1' -Comment "Kiri: containers answer at <name>.$Suffix"
  }
  Clear-DnsClientCache
  exit 0
}

throw "kiri-admin.ps1 needs -Wsl or -Dns"
