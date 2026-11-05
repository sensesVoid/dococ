param(
    [string]$Model = "C:\Users\geral\dococ\models\laya\multilingual",
    [int]$Reps = 10,
    [int]$N = 5,
    [int]$Threads = 0,
    [string]$Exe = "target\release\examples\laya_bench.exe"
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$exePath = Join-Path $root $Exe
if (-not (Test-Path $exePath)) {
    throw "Missing $exePath. Build it with: cargo build --release --example laya_bench -p dococ-laya"
}

$out = Join-Path $root "models\laya\bench.out"
$err = Join-Path $root "models\laya\bench.err"

$env:DOCOC_LAYA_MODEL = $Model
$env:DOCOC_LAYA_BENCH_REPS = "$Reps"
$env:DOCOC_LAYA_BENCH_N = "$N"
if ($Threads -gt 0) {
    $env:DOCOC_LAYA_THREADS = "$Threads"
} else {
    Remove-Item Env:DOCOC_LAYA_THREADS -ErrorAction SilentlyContinue
}

$process = Start-Process -FilePath $exePath -PassThru -NoNewWindow `
    -RedirectStandardOutput $out -RedirectStandardError $err

$peak = 0
while (-not $process.HasExited) {
    try {
        $process.Refresh()
        if ($process.PeakWorkingSet64 -gt $peak) {
            $peak = $process.PeakWorkingSet64
        }
    } catch {
        # The process may exit between HasExited and Refresh.
    }
    Start-Sleep -Milliseconds 100
}

if (Test-Path $out) {
    Get-Content $out
}
if (Test-Path $err) {
    $errText = Get-Content $err -Raw
    if ($errText) {
        Write-Error $errText
    }
}

"peak_rss_mb=" + [math]::Round($peak / 1MB, 1)
