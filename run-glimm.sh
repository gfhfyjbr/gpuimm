#!/bin/bash
set -euo pipefail
root="$(cd "$(dirname "$0")" && pwd)"
cd "$root"
# Use the SDK belonging to the selected Xcode, not a newer incompatible CLT SDK.
if [[ "$(uname -s)" == "Darwin" ]]; then
    export SDKROOT="${SDKROOT:-$(xcrun --sdk macosx --show-sdk-path)}"
fi
# --dev follows the platform's shader source: each save recompiles it in the running playground.
if [[ "${1:-}" == "--dev" ]]; then
    shift
    if [[ "$(uname -s)" == "Darwin" ]]; then
        export GPUI_GLIMM_SHADER_DIR="${GPUI_GLIMM_SHADER_DIR:-$root/crates/gpui_apple/src/fast}"
    else
        export GPUI_GLIMM_SHADER_DIR="${GPUI_GLIMM_SHADER_DIR:-$root/crates/gpui_wgpu/src/fast}"
    fi
    echo "following $GPUI_GLIMM_SHADER_DIR" >&2
fi
exec cargo run -p gpui_perf --example glimm --features gpui_platform_fast/test-support -- "$@"
