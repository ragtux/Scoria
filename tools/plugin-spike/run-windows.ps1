# The spike on Windows (x86_64-pc-windows-msvc): builds a subset of run.sh's
# hosts and runs the bench. Copy the folder without target/ to the box, with
# out/ holding the two guests run.sh built, then from a shell that has run
# vcvars64.bat:
#
#   powershell -NoProfile -ExecutionPolicy Bypass -File run-windows.ps1 [-Build name,name] [-Bench]
#
# With no switches it builds every variant and benches them. -Build rebuilds
# only the named ones (appending to the log); -Bench only benches. An ssh
# session that outlives its timeout takes the script with it, so the builds
# can be split across sessions.
param([string[]]$Build, [switch]$Bench)
$ErrorActionPreference = "Stop"
$here = $PSScriptRoot
$out = Join-Path $here "out"
$variants = @(
  @("baseline", "host-baseline", @()),
  @("wit-cranelift", "host-wit", @("--features", "cranelift")),
  @("wit-cranelift-parallel", "host-wit", @("--features", "cranelift-parallel")),
  @("wit-winch", "host-wit", @("--features", "winch")),
  @("wit-runtime", "host-wit", @("--features", "runtime")),
  @("extism-minimal", "host-extism", @("--no-default-features"))
)
$log = Join-Path $out "windows.txt"
$all = -not $Build -and -not $Bench
if ($all) { Set-Content $log "" }
foreach ($v in $variants) {
  $name, $dir, $flags = $v
  if (-not $all -and $Build -notcontains $name) { continue }
  $tgt = Join-Path $here "target\$name"
  if (Test-Path $tgt) { Remove-Item -Recurse -Force $tgt }
  Push-Location (Join-Path $here $dir)
  $env:CARGO_TARGET_DIR = $tgt
  cargo fetch -q
  $clean = Measure-Command { cargo build --release -q @flags }
  (Get-Item "src\main.rs").LastWriteTime = Get-Date
  $incr = Measure-Command { cargo build --release -q @flags }
  Pop-Location
  $bin = Join-Path $tgt "release\spike-$dir.exe"
  $size = (Get-Item $bin).Length
  $line = "$name size=$size clean_s={0:N1} incr_s={1:N1}" -f $clean.TotalSeconds, $incr.TotalSeconds
  Add-Content $log $line
  Write-Output $line
}
if (-not $all -and -not $Bench) { exit 0 }
function Bin($name, $dir) { Join-Path $here "target\$name\release\spike-$dir.exe" }
& (Bin "wit-cranelift" "host-wit") (Join-Path $out "guest-wit.component.wasm") --precompile (Join-Path $out "guest-wit.win.cwasm")
foreach ($v in $variants) {
  $name, $dir, $flags = $v
  # The guest path is passed as one argument (splatting a lone string would
  # pass its characters), and the baseline takes none.
  $guest = switch -Wildcard ($name) {
    "baseline" { $null }
    "wit-runtime" { Join-Path $out "guest-wit.win.cwasm" }
    "wit-*" { Join-Path $out "guest-wit.component.wasm" }
    "extism-*" { Join-Path $out "guest-extism.wasm" }
  }
  $guestArgs = if ($guest) { , $guest } else { @() }
  Add-Content $log "== $name"
  & (Bin $name $dir) $guestArgs | Where-Object { $_ -notmatch '^(command|notice)=' } | ForEach-Object { Add-Content $log $_; $_ }
}
