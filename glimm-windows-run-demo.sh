#!/bin/bash
set -euo pipefail
root="$(cd "$(dirname "$0")" && pwd)"
args=()
previous=''
for argument in "$@"; do
    if [[ "$previous" == '--capture' || "$previous" == '--config' ]]; then
        args+=("Z:\\workspace\\$(basename "$argument")")
    else
        args+=("$argument")
    fi
    previous="$argument"
done
export TART_HOME="$root/glimm-tart-windows-vms"
export TART_NO_AUTO_PRUNE=1
exec "$root/glimm-tart-windows-runtime/tart.app/Contents/MacOS/tart" exec glimm-windows \
    'Z:\workspace\target\glimm-windows\x86_64-pc-windows-gnu\debug\examples\glimm.exe' "${args[@]}"
