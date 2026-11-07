#!/bin/bash
set -euo pipefail
root="$(cd "$(dirname "$0")" && pwd)"
cd "$root/gpui-fast"
# Use the SDK belonging to the selected Xcode, not a newer incompatible CLT SDK.
if [[ "$(uname -s)" == "Darwin" ]]; then
    export SDKROOT="${SDKROOT:-$(xcrun --sdk macosx --show-sdk-path)}"
fi
exec cargo run -p gpui_perf --example glimm --features gpui_platform_fast/test-support -- "$@"
