# Installs target/Kiri-<version>-x86_64.msi (-arm64.msi on Windows on Arm) silently (no launch, so first-run
# setup does not touch WSL), checks the per-user install, uninstalls silently
# and checks it is gone. Safe on a PC where Kiri's setup never completed:
# the uninstall step only undoes setup when setup.json exists.
#
#   pwsh -File installer/windows/verify.ps1 [-Msi path]

param([string]$Msi)
$ErrorActionPreference = 'Continue'
$root = Resolve-Path (Join-Path $PSScriptRoot '..\..')
# This PC's own MSI: arm64 on Windows on Arm (native PowerShell says ARM64 there).
$arch = if (@($env:PROCESSOR_ARCHITEW6432, $env:PROCESSOR_ARCHITECTURE) -contains 'ARM64') { 'arm64' } else { 'x86_64' }
if (-not $Msi) { $Msi = (Get-ChildItem (Join-Path $root "target\Kiri-*-$arch.msi") | Sort-Object LastWriteTime | Select-Object -Last 1).FullName }
$logs = Join-Path $root 'target\msi-verify'
New-Item -ItemType Directory -Force $logs | Out-Null

$dir = Join-Path $env:LOCALAPPDATA 'Kiri'
$exe = Join-Path $dir 'kiri-win.exe'
$shortcut = Join-Path $env:APPDATA 'Microsoft\Windows\Start Menu\Programs\Kiri.lnk'
$files = 'kiri-win.exe', 'kiri', 'kiri-init', 'kiri-linux.tar.gz', 'install.sh', 'kiri.ps1', 'setup-tools.ps1', 'kiri-admin.ps1', 'bin\docker.exe', 'bin\cli-plugins\docker-compose.exe', 'bin\cli-plugins\docker-buildx.exe'

$elevated = ([Security.Principal.WindowsPrincipal][Security.Principal.WindowsIdentity]::GetCurrent()).IsInRole([Security.Principal.WindowsBuiltInRole]::Administrator)
"msi: $Msi"
"shell elevated: $elevated (a per-user install is only a real test when False)"
if (Test-Path (Join-Path $dir 'setup.json')) { "NOTE: Kiri's setup completed on this PC; uninstalling will undo it (engine in WSL, PATH, docker context)." }

function Check([string]$what, [bool]$ok) { "{0,-6} {1}" -f ($(if ($ok) { '[ok]' } else { '[FAIL]' }), $what); if (-not $ok) { $script:failed = $true } }
# Windows Installer registers a per-user product's Apps & features row under
# HKLM\...\Uninstall\{ProductCode} on some Windows builds and under HKCU on
# others; either is the same per-user install (files under %LOCALAPPDATA%,
# the product's own keys in HKCU, nothing under Program Files).
function ArpEntry {
    Get-ChildItem 'HKCU:\Software\Microsoft\Windows\CurrentVersion\Uninstall', 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall' -ErrorAction SilentlyContinue |
        ForEach-Object { Get-ItemProperty $_.PSPath } | Where-Object { $_.DisplayName -eq 'Kiri' -and $_.Publisher -eq 'Kiri' } | Select-Object -First 1
}

$script:failed = $false
Get-Process kiri-win -ErrorAction SilentlyContinue | Stop-Process -Force
$p = Start-Process msiexec -ArgumentList "/i `"$Msi`" /qn /norestart /l*v `"$logs\install.log`"" -Wait -PassThru
Check "msiexec /i exit code $($p.ExitCode)" ($p.ExitCode -eq 0)
foreach ($f in $files) { Check "installed $f" (Test-Path (Join-Path $dir $f)) }
Check "Start Menu shortcut" (Test-Path $shortcut)
if (Test-Path $shortcut) {
    $sh = (New-Object -ComObject WScript.Shell).CreateShortcut($shortcut)
    Check "shortcut target is $exe" ($sh.TargetPath -ieq $exe)
}
$arp = ArpEntry
Check "Apps & features entry" ($null -ne $arp)
if ($arp) {
    "       $($arp.PSPath -replace '^.*::') DisplayVersion=$($arp.DisplayVersion) InstallLocation=$($arp.InstallLocation)"
    Check "version $($arp.DisplayVersion)" ($arp.DisplayVersion -match '^\d+\.\d+\.\d+$')
}
Check "HKCU\Software\Kiri\InstallDir" ((Get-ItemProperty 'HKCU:\Software\Kiri' -ErrorAction SilentlyContinue).InstallDir -ieq ($dir + '\'))
Check "nothing under Program Files" (-not (Test-Path "$env:ProgramFiles\Kiri"))
$p = Start-Process msiexec -ArgumentList "/x `"$Msi`" /qn /norestart /l*v `"$logs\uninstall.log`"" -Wait -PassThru
Check "msiexec /x exit code $($p.ExitCode)" ($p.ExitCode -eq 0)
foreach ($f in $files) { Check "removed $f" (-not (Test-Path (Join-Path $dir $f))) }
Check "Start Menu shortcut gone" (-not (Test-Path $shortcut))
Check "Apps & features entry gone" ($null -eq (ArpEntry))
Check "HKCU\Software\Kiri gone" (-not (Test-Path 'HKCU:\Software\Kiri'))
"`nlogs: $logs"
if ($script:failed) { "RESULT: FAILED"; exit 1 } else { "RESULT: PASSED"; exit 0 }
