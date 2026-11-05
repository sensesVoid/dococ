param(
    [ValidateSet("multilingual", "english", "typed-decisions", "all")]
    [string]$Model = "multilingual",
    [switch]$Verify
)

$ErrorActionPreference = "Stop"

$root = Split-Path -Parent $PSScriptRoot
$python = Join-Path $root ".venv-laya\Scripts\python.exe"
$exporter = Join-Path $root "third_party\laya\laya-rust\tools\export_onnx.py"
$out = Join-Path $root "models\laya"

if (-not (Test-Path $python)) {
    throw "Missing $python. Create .venv-laya and install the Laya export dependencies first."
}

$pyArgs = @($exporter, "--model", $Model, "--out", $out)
if ($Verify) {
    $pyArgs += "--verify"
}

& $python @pyArgs
