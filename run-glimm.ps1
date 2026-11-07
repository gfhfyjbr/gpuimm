$ErrorActionPreference = 'Stop'
Push-Location (Join-Path $PSScriptRoot 'gpui-fast')
try {
    & cargo run -p gpui_perf --example glimm --features gpui_platform_fast/test-support -- @args
    $code = $LASTEXITCODE
} finally {
    Pop-Location
}
exit $code
