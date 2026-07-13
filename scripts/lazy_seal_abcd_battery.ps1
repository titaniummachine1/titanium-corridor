# A/B/C/D lazy-seal validation battery (post ACE-Board coordinate fix).
# Treat any benchmark from before the mapping fix as invalid.
param(
    [string[]]$Positions = @("startpos", "c3h-midgame", "wall-maze", "endgame-c5"),
    [int[]]$Depths = @(6, 8, 10, 12),
    [string]$OutDir = "runs/lazy_seal_abcd_postfix"
)

$ErrorActionPreference = "Stop"
$EngineRoot = Split-Path $PSScriptRoot -Parent
Set-Location $EngineRoot

$env:RUSTFLAGS = "-C target-cpu=native"
$env:TITANIUM_BENCH_ENGINE = "titanium-v17"

$Bench = Join-Path $EngineRoot "target\release\search_bench.exe"
if (-not (Test-Path $Bench)) {
    Write-Host "Building search_bench (native release)..."
    cargo build --release -p titanium --bin search_bench
    if ($LASTEXITCODE -ne 0) { exit $LASTEXITCODE }
}

$RunRoot = Join-Path $EngineRoot $OutDir
New-Item -ItemType Directory -Force -Path $RunRoot | Out-Null

$Configs = @(
    @{ Name = "A"; Lazy = "0"; Seal = $null; Label = "v17-full-legal" },
    @{ Name = "B"; Lazy = "1"; Seal = "eager"; Label = "lazy-eager-heavy" },
    @{ Name = "C"; Lazy = "1"; Seal = "deferred"; Label = "lazy-deferred-heavy" },
    @{ Name = "D"; Lazy = "1"; Seal = "legacy"; Label = "lazy-legacy-seal" }
)

function Parse-LazySealStats([string]$Line) {
    $h = [ordered]@{}
    if ($Line -match "walls_gen=(\d+)") { $h.walls_gen = [int64]$Matches[1] }
    if ($Line -match "walls=(\d+)") { $h.walls_examined = [int64]$Matches[1] }
    if ($Line -match "safe=(\d+)") { $h.topo_skips = [int64]$Matches[1] }
    if ($Line -match "risky=(\d+)") { $h.risky_walls = [int64]$Matches[1] }
    if ($Line -match "illegal_risky=(\d+)") { $h.illegal_rejected = [int64]$Matches[1] }
    if ($Line -match "heavy_checks=(\d+)") { $h.pbff_calls = [int64]$Matches[1] }
    if ($Line -match "heavy_ctx=(\d+)") { $h.heavy_ctx = [int64]$Matches[1] }
    return $h
}

$Results = @()
$ParityFails = @()

foreach ($cfg in $Configs) {
    $env:TITANIUM_BENCH_LAZY_WALLS = $cfg.Lazy
    if ($cfg.Seal) { $env:TITANIUM_LAZY_SEAL_MODE = $cfg.Seal }
    else { Remove-Item Env:TITANIUM_LAZY_SEAL_MODE -ErrorAction SilentlyContinue }

    foreach ($pos in $Positions) {
        foreach ($depth in $Depths) {
            $tag = "$($cfg.Name)_${pos}_d$depth"
            $logPath = Join-Path $RunRoot "$tag.log"
            Write-Host "[$tag] $($cfg.Label) depth=$depth position=$pos"

            $prevEap = $ErrorActionPreference
            $ErrorActionPreference = "Continue"
            try {
                $out = @(& $Bench depth --position $pos --depth $depth 2>&1 | ForEach-Object { "$_" })
            } finally {
                $ErrorActionPreference = $prevEap
            }
            $out | Set-Content -Encoding utf8 $logPath

            $jsonLine = ($out | Where-Object { $_ -match '^\s*\{' } | Select-Object -Last 1)
            if (-not $jsonLine) {
                Write-Warning "No JSON line for $tag"
                continue
            }
            $j = $jsonLine | ConvertFrom-Json
            $lazyLine = ($out | Where-Object { $_ -match "^lazy_seal " } | Select-Object -Last 1)
            $lazy = if ($lazyLine) { Parse-LazySealStats $lazyLine } else { @{} }

            $row = [ordered]@{
                config      = $cfg.Name
                label       = $cfg.Label
                position    = $pos
                depth       = $depth
                best_move   = $j.move
                score       = $j.score
                nodes       = $j.nodes
                nps         = [math]::Round($j.nps, 0)
                elapsed_ms  = $j.elapsed_ms
                walls_gen   = $lazy.walls_gen
                walls_exam  = $lazy.walls_examined
                topo_skips  = $lazy.topo_skips
                risky       = $lazy.risky_walls
                pbff        = $lazy.pbff_calls
                heavy_ctx   = $lazy.heavy_ctx
                illegal_rej = $lazy.illegal_rejected
            }
            $Results += [pscustomobject]$row
        }
    }
}

$csvPath = Join-Path $RunRoot "summary.csv"
$Results | Export-Csv -NoTypeInformation -Path $csvPath

Write-Host ""
Write-Host "=== B/C/D fixed-depth parity (move, score, nodes) ==="
foreach ($pos in $Positions) {
    foreach ($depth in $Depths) {
        $bcd = $Results | Where-Object { $_.config -in @("B","C","D") -and $_.position -eq $pos -and $_.depth -eq $depth }
        if ($bcd.Count -lt 3) { continue }
        $ref = $bcd | Where-Object { $_.config -eq "B" } | Select-Object -First 1
        foreach ($r in ($bcd | Where-Object { $_.config -ne "B" })) {
            if ($r.best_move -ne $ref.best_move -or $r.score -ne $ref.score -or $r.nodes -ne $ref.nodes) {
                $msg = "MISMATCH $pos d$depth : B=$($ref.best_move)/$($ref.score)/$($ref.nodes) vs $($r.config)=$($r.best_move)/$($r.score)/$($r.nodes)"
                Write-Host $msg
                $ParityFails += $msg
            }
        }
    }
}
if ($ParityFails.Count -eq 0) {
    Write-Host "B/C/D parity: PASS (all compared runs identical)"
} else {
    Write-Host "B/C/D parity: FAIL ($($ParityFails.Count) mismatches) - do NOT start Elo"
}

Write-Host ""
Write-Host "=== A vs C score plausibility (lazy deferred) ==="
foreach ($pos in $Positions) {
    foreach ($depth in $Depths) {
        $a = $Results | Where-Object { $_.config -eq "A" -and $_.position -eq $pos -and $_.depth -eq $depth } | Select-Object -First 1
        $c = $Results | Where-Object { $_.config -eq "C" -and $_.position -eq $pos -and $_.depth -eq $depth } | Select-Object -First 1
        if ($a -and $c) {
            $delta = [math]::Abs($a.score - $c.score)
            Write-Host ("{0,-14} d{1,2}  A: {2,6} {3,-5} n={4,8}  C: {5,6} {6,-5} n={7,8}  abs_score_delta={8}" -f `
                $pos, $depth, $a.score, $a.best_move, $a.nodes, $c.score, $c.best_move, $c.nodes, $delta)
        }
    }
}

Write-Host ""
Write-Host "Summary CSV: $csvPath"
if ($ParityFails.Count -gt 0) { exit 2 }
