# Run titanium perft/bench pinned to the highest-index core among the least-loaded CPUs.
# Usage: .\scripts\bench-pinned.ps1 [-PerftDepth 4] [-BenchDepth 3] [-BenchIters 20] [-Runs 3]

param(
    [int]$PerftDepth = 4,
    [int]$BenchDepth = 3,
    [int]$BenchIters = 20,
    [int]$Runs = 3
)

$ErrorActionPreference = "Stop"
$EngineRoot = Split-Path -Parent $PSScriptRoot
Set-Location $EngineRoot

function Get-PinnedCore {
    $samples = Get-Counter '\Processor(*)\% Processor Time' -SampleInterval 1 -MaxSamples 2
    $cores = @()
    foreach ($s in $samples.CounterSamples) {
        if ($s.InstanceName -eq '_Total') { continue }
        if ($s.InstanceName -match '^\d+$') {
            $cores += [pscustomobject]@{
                Id   = [int]$s.InstanceName
                Load = $s.CookedValue
            }
        }
    }
    if ($cores.Count -eq 0) {
        $n = (Get-CimInstance Win32_ComputerSystem).NumberOfLogicalProcessors
        return [Math]::Max(0, $n - 1)
    }
    $minLoad = ($cores | Measure-Object -Property Load -Minimum).Minimum
    $slack = 8.0
    $candidates = $cores | Where-Object { $_.Load -le ($minLoad + $slack) }
    if ($candidates.Count -eq 0) { $candidates = $cores }
    $picked = $candidates | Sort-Object Id -Descending | Select-Object -First 1
    Write-Host ("core pick: id={0} load={1:N1}% (min={2:N1}%, candidates={3})" -f `
            $picked.Id, $picked.Load, $minLoad, $candidates.Count)
    return $picked.Id
}

$core = Get-PinnedCore
$env:TITANIUM_PIN_CORE = "$core"
Remove-Item Env:TITANIUM_PIN_LAST -ErrorAction SilentlyContinue

Write-Host "building release..."
cargo build --release --bin titanium --quiet
if ($LASTEXITCODE -ne 0) { throw "cargo build failed" }

$Titanium = Join-Path $EngineRoot "target\release\titanium.exe"
if (-not (Test-Path $Titanium)) { throw "missing $Titanium" }

Write-Host ""
Write-Host "=== perft $PerftDepth (pinned core $core, $Runs runs) ==="
for ($i = 1; $i -le $Runs; $i++) {
    Write-Host "--- run $i ---"
    & $Titanium perft $PerftDepth
    if ($LASTEXITCODE -ne 0) { throw "perft run $i failed" }
}

Write-Host ""
Write-Host "=== bench $BenchDepth $BenchIters (pinned core $core) ==="
& $Titanium bench $BenchDepth $BenchIters
if ($LASTEXITCODE -ne 0) { throw "bench failed" }
