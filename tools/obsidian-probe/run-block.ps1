# run-block.sh for Windows: one CDP probe script against a fresh scratch vault,
# in the same phases (fixtures, run, after-restart), always under a data
# directory of its own so the person's Obsidian and its obsidian.json are never
# touched.
#
#   $env:USER_DATA_DIR = "$env:TEMP\scoria-probe-udd"; $env:CDP_PORT = 9231
#   pwsh tools/obsidian-probe/run-block.ps1 tools/obsidian-probe/theme-plugins.mjs results.json
#
# Obsidian holds its single-instance lock per --user-data-dir, so this copy runs
# beside a desktop one. The newest obsidian-*.asar under %APPDATA%\obsidian is
# copied in once so the run is on the version the person runs, and obsidian.json
# carries `updateDisabled`: otherwise the first launch fetches the newest release
# into the data directory, deletes the copied asar, and every later launch runs
# a version the person does not (seen 2026-11-04: 1.14.5 became 1.15.0 mid-probe).
# Only the process this script started is killed on the way out, by its PID,
# with its children.
param(
  [Parameter(Mandatory = $true)][string]$Script,
  [Parameter(Mandatory = $true)][string]$Results,
  [string]$Vault
)
$ErrorActionPreference = 'Stop'
$Script = (Resolve-Path $Script).Path
$Results = [System.IO.Path]::GetFullPath($Results)
if (-not $Vault) { $Vault = Join-Path ([System.IO.Path]::GetTempPath()) ("scoria-probe-vault-" + [guid]::NewGuid().ToString('N').Substring(0, 8)) }
$Vault = [System.IO.Path]::GetFullPath($Vault)
$Udd = $env:USER_DATA_DIR
if (-not $Udd) { $Udd = Join-Path ([System.IO.Path]::GetTempPath()) 'scoria-probe-udd' }
$Port = if ($env:CDP_PORT) { $env:CDP_PORT } else { '9222' }
$Exe = Join-Path $env:LOCALAPPDATA 'Programs\Obsidian\Obsidian.exe'
$env:CDP_BASE = "http://127.0.0.1:$Port"
$env:VAULT = $Vault
$env:RESULTS = $Results

New-Item -ItemType Directory -Force $Udd | Out-Null
New-Item -ItemType Directory -Force (Join-Path $Vault '.obsidian') | Out-Null
if (-not (Test-Path (Join-Path $Vault '.obsidian\app.json'))) { Set-Content -NoNewline -Path (Join-Path $Vault '.obsidian\app.json') -Value '{}' }
if (-not (Get-ChildItem $Udd -Filter 'obsidian-*.asar' -ErrorAction SilentlyContinue)) {
  $newest = Get-ChildItem (Join-Path $env:APPDATA 'obsidian') -Filter 'obsidian-*.asar' -ErrorAction SilentlyContinue |
    Sort-Object { [version]($_.BaseName -replace '^obsidian-', '') } | Select-Object -Last 1
  if ($newest) { Copy-Item $newest.FullName $Udd }
}
# A fresh 16-hex id per vault: a reused id inherits that id's IndexedDB cache.
$id = -join ((1..16) | ForEach-Object { '{0:x}' -f (Get-Random -Maximum 16) })
@{ updateDisabled = $true; vaults = @{ $id = @{ path = $Vault; ts = 1792373000000; open = $true } } } | ConvertTo-Json -Depth 4 -Compress |
  Set-Content -NoNewline -Path (Join-Path $Udd 'obsidian.json')

$cdp = (Join-Path (Split-Path $Script) 'cdp.mjs') -replace '\\', '/'
$proc = $null
function Launch {
  $script:proc = Start-Process -FilePath $Exe -ArgumentList "--user-data-dir=`"$Udd`"", "--remote-debugging-port=$Port" -PassThru
  # The answer is printed rather than given as the exit status: node on Windows
  # can abort in libuv (UV_HANDLE_CLOSING) when it exits with a socket closing.
  $said = node -e "import('file:///$cdp').then(async (m) => { await m.waitForObsidian(90000); const c = await m.connect(); const p = await c.vaultPath(); const n = (s) => s.replace(/[\\/]+$/, '').toLowerCase(); console.log(n(p) === n(process.env.VAULT) ? 'OURS' : 'WRONG ' + p); process.exit(0); }).catch((e) => { console.log('FAILED ' + e.message); process.exit(1); })" 2>$null
  if (-not ($said -contains 'OURS')) { throw "[run-block] Obsidian did not open $Vault ($said)" }
}
function Gone { try { Invoke-WebRequest -UseBasicParsing -TimeoutSec 1 "$env:CDP_BASE/json" | Out-Null; return $false } catch { return $true } }
function Stop-Ours { if ($script:proc -and -not $script:proc.HasExited) { & taskkill /PID $script:proc.Id /T /F | Out-Null; Start-Sleep -Seconds 2 } }

try {
  if (-not (Gone)) { throw "[run-block] something already answers on port $Port; pick another CDP_PORT" }
  Write-Host '[run-block] phase fixtures'; node $Script fixtures
  Write-Host '[run-block] launch'; Launch
  Write-Host '[run-block] phase run'; node $Script run
  if ($LASTEXITCODE -ne 0) { Write-Host '[run-block] run phase exited non-zero' }
  Start-Sleep -Seconds 2
  if (Gone) {
    Write-Host '[run-block] Obsidian quit during run; relaunching for after-restart'
    Stop-Ours; Launch
    Write-Host '[run-block] phase after-restart'; node $Script after-restart
  }
} finally {
  Stop-Ours
  Write-Host "[run-block] done: $Results; vault left at $Vault"
}
