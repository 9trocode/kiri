# Kiri's Windows icons, from icon/kiri-app-icon-windows.svg (generate.py):
# Windows draws icons to the edge of their square, so that variant's tile
# fills the canvas and its mark is drawn larger than the Mac icon's, which
# keeps Apple's margin and looks a size small in the taskbar. Rendered with
# Edge (headless, on every Windows), then scaled to each size Windows uses.
# Writes icon/windows/kiri-{16..256}.png and kiri.ico.
#   powershell -File brand/windows-icons.ps1
Add-Type -AssemblyName System.Drawing
$svg = Join-Path $PSScriptRoot "icon\kiri-app-icon-windows.svg"
$out = Join-Path $PSScriptRoot "icon\windows"
New-Item -ItemType Directory -Force $out | Out-Null

$edge = @("${env:ProgramFiles(x86)}\Microsoft\Edge\Application\msedge.exe", "$env:ProgramFiles\Microsoft\Edge\Application\msedge.exe") | Where-Object { Test-Path $_ } | Select-Object -First 1
if (-not $edge) { throw "Microsoft Edge is needed to render the SVG" }
$master = Join-Path $env:TEMP "kiri-icon-1024.png"
$profile = Join-Path $env:TEMP "kiri-icon-edge"
Remove-Item $master -ErrorAction SilentlyContinue
$url = "file:///" + ($svg -replace '\\', '/')
& $edge --headless=new --disable-gpu --hide-scrollbars --force-device-scale-factor=1 --default-background-color=00000000 `
  "--user-data-dir=$profile" --window-size=1024,1024 "--screenshot=$master" $url 2>$null | Out-Null
for ($i = 0; $i -lt 50 -and -not (Test-Path $master); $i++) { Start-Sleep -Milliseconds 200 }
if (-not (Test-Path $master)) { throw "Edge did not render $svg" }

$src = [System.Drawing.Bitmap]::FromFile($master)
$sizes = 16, 20, 24, 32, 40, 48, 64, 256
$pngs = @{}
foreach ($s in $sizes) {
  $bmp = New-Object System.Drawing.Bitmap $s, $s
  $g = [System.Drawing.Graphics]::FromImage($bmp)
  $g.InterpolationMode = "HighQualityBicubic"; $g.PixelOffsetMode = "HighQuality"; $g.SmoothingMode = "HighQuality"; $g.CompositingQuality = "HighQuality"
  $g.DrawImage($src, [System.Drawing.Rectangle]::new(0, 0, $s, $s), [System.Drawing.Rectangle]::new(0, 0, $src.Width, $src.Height), "Pixel")
  $g.Dispose()
  $file = Join-Path $out "kiri-$s.png"
  $bmp.Save($file, [System.Drawing.Imaging.ImageFormat]::Png); $bmp.Dispose()
  $pngs[$s] = [System.IO.File]::ReadAllBytes($file)
}
$src.Dispose()

# An .ico of PNG images (Windows Vista and later read these at any size).
$ico = New-Object System.IO.MemoryStream
$w = New-Object System.IO.BinaryWriter $ico
$w.Write([uint16]0); $w.Write([uint16]1); $w.Write([uint16]$sizes.Count)
$offset = 6 + 16 * $sizes.Count
foreach ($s in $sizes) {
  $dim = if ($s -ge 256) { 0 } else { $s }
  $w.Write([byte]$dim); $w.Write([byte]$dim); $w.Write([byte]0); $w.Write([byte]0)
  $w.Write([uint16]1); $w.Write([uint16]32); $w.Write([uint32]$pngs[$s].Length); $w.Write([uint32]$offset)
  $offset += $pngs[$s].Length
}
foreach ($s in $sizes) { $w.Write($pngs[$s]) }
[System.IO.File]::WriteAllBytes((Join-Path $out "kiri.ico"), $ico.ToArray())
"rendered $svg; wrote $($sizes.Count) PNGs and kiri.ico to $out"
