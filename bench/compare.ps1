# Times the hot paths of an engine from the Windows docker CLI: container
# start, exec, and 200 MB each way. Run it against Kiri's pipe and against
# Docker Desktop's, back to back, and compare.
#
#   bench\compare.ps1 -Engine npipe:////./pipe/kiri -Label kiri
#   bench\compare.ps1 -Engine npipe:////./pipe/docker_engine -Label desktop
#
# Medians of 20 for the latencies. The "in" figure only means something if
# every byte arrived; the count the container saw is printed next to it.
param(
  [string]$Engine = "npipe:////./pipe/kiri",
  [string]$Label = "kiri",
  [string]$Docker = "docker"
)
$env:DOCKER_HOST = $Engine
& $Docker pull -q alpine 2>$null | Out-Null
1..3 | ForEach-Object { & $Docker run --rm alpine true 2>$null | Out-Null }
function median($xs) { $s = $xs | Sort-Object; $s[[int]($s.Count / 2)] }
$t = @(); 1..20 | ForEach-Object { $sw = [Diagnostics.Stopwatch]::StartNew(); & $Docker run --rm alpine true 2>$null | Out-Null; $t += $sw.ElapsedMilliseconds }
$run = median $t; $runMin = ($t | Measure-Object -Minimum).Minimum
$name = "bench-$PID"
& $Docker run -d --name $name alpine sleep 300 2>$null | Out-Null
$t = @(); 1..20 | ForEach-Object { $sw = [Diagnostics.Stopwatch]::StartNew(); & $Docker exec $name true 2>$null | Out-Null; $t += $sw.ElapsedMilliseconds }
$exec = median $t; $execMin = ($t | Measure-Object -Minimum).Minimum
$big = Join-Path $env:TEMP "kiri-bench-200mb.bin"
if (-not (Test-Path $big) -or (Get-Item $big).Length -ne 200000000) { [IO.File]::WriteAllBytes($big, (New-Object byte[] 200000000)) }
# cmd does the redirection: PowerShell's own pipeline is far slower than either engine.
$sw = [Diagnostics.Stopwatch]::StartNew(); cmd /c "`"$Docker`" run --rm alpine sh -c `"head -c 200000000 /dev/zero`" > NUL"; $out = $sw.ElapsedMilliseconds
$sw = [Diagnostics.Stopwatch]::StartNew(); $seen = (cmd /c "`"$Docker`" run --rm -i alpine wc -c < `"$big`"").Trim(); $in = $sw.ElapsedMilliseconds
& $Docker rm -f $name 2>$null | Out-Null
"{0,-10} run {1,3} ms (min {2,3})  exec {3,3} ms (min {4,3})  200MB out {5,5} ms = {6,4} MB/s  200MB in {7,5} ms = {8,4} MB/s (container saw {9} bytes)" -f `
  $Label, $run, $runMin, $exec, $execMin, $out, [int](200000 / [Math]::Max($out, 1)), $in, [int](200000 / [Math]::Max($in, 1)), $seen
