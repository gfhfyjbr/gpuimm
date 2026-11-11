$ErrorActionPreference = 'Stop'
$rest = $args
# -dev (or --dev) follows the HLSL source: each save recompiles it in the running playground.
if ($rest.Count -gt 0 -and ($rest[0] -eq '--dev' -or $rest[0] -eq '-dev')) {
    $rest = @($rest | Select-Object -Skip 1)
    if (-not $env:GPUI_GLIMM_SHADER_DIR) {
        $env:GPUI_GLIMM_SHADER_DIR = Join-Path $PSScriptRoot 'crates\gpui_windows\src\fast'
    }
    Write-Host "following $env:GPUI_GLIMM_SHADER_DIR"
}
Push-Location $PSScriptRoot
try {
    & cargo run -p gpui_perf --example glimm --features gpui_platform_fast/test-support -- @rest
    $code = $LASTEXITCODE
} finally {
    Pop-Location
}
exit $code
