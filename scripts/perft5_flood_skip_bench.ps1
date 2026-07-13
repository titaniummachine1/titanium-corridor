# Identical perft(5) bench: prewarm -> readyok -> timed perft only.
# Hard 20s wall clock in the binary — exit 1 if exceeded (anchor mode will always hit it).
# Requires release binary built with RUSTFLAGS=-C target-cpu=native.

$ErrorActionPreference = "Stop"
$EngineRoot = Split-Path $PSScriptRoot -Parent
$Exe = Join-Path $EngineRoot "target\release\titanium.exe"
$ExpectedNodes = 28837934502

if (-not (Test-Path $Exe)) {
    throw "Missing $Exe - build first: cargo build --release --bin titanium"
}

function Invoke-PerftBenchRun {
    param(
        [string]$Mode
    )

    $env:RUSTFLAGS = '-C target-cpu=native'
    $env:TITANIUM_BENCH = '1'
    if ($Mode -eq 'anchor') {
        $env:TITANIUM_WALL_FLOOD_SKIP = 'anchor'
    } else {
        Remove-Item Env:\TITANIUM_WALL_FLOOD_SKIP -ErrorAction SilentlyContinue
    }

    $lines = & $Exe perft-bench 5 2>&1
    if ($LASTEXITCODE -ne 0) {
        throw "perft-bench failed for mode=${Mode} exit=$LASTEXITCODE"
    }

    $ready = $false
    $result = $null
    foreach ($line in $lines) {
        Write-Output $line
        if ($line -eq 'readyok') {
            $ready = $true
        }
        if ($line -match '^perft_bench ') {
            $result = $line
        }
    }

    if (-not $ready) {
        throw "mode=${Mode} never saw readyok"
    }
    if (-not $result) {
        throw "mode=${Mode} missing perft_bench result line"
    }

    if ($result -notmatch 'nodes=(\d+)') {
        throw "mode=${Mode} could not parse nodes from: $result"
    }
    $nodes = [int64]$Matches[1]
    if ($nodes -ne $ExpectedNodes) {
        throw "mode=${Mode} nodes=$nodes expected=$ExpectedNodes"
    }

    return [pscustomobject]@{
        Mode  = $Mode
        Ready = $ready
        Line  = $result
        Nodes = $nodes
    }
}

Write-Output "=== perft(5) flood-skip bench (fresh process, readyok, 20s cap on topo) ==="
$topo = Invoke-PerftBenchRun -Mode 'topo'
Write-Output ""
Write-Output "=== anchor baseline (expected TIMEOUT >20s — skip if exit 1) ==="
try {
    $anchor = Invoke-PerftBenchRun -Mode 'anchor'
    Write-Output $anchor.Line
} catch {
    Write-Output "anchor run aborted: $_"
}
Write-Output ""
Write-Output "=== summary ==="
Write-Output $topo.Line
