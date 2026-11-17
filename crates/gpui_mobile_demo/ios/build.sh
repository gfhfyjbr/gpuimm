#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../../.." && pwd)"
target="${GPUIMM_RUST_TARGET:-aarch64-apple-ios-sim}"
case "$target" in
  aarch64-apple-ios-sim) sdk=iphonesimulator; arch=arm64 ;;
  x86_64-apple-ios) sdk=iphonesimulator; arch=x86_64 ;;
  aarch64-apple-ios) sdk=iphoneos; arch=arm64 ;;
  *) echo "Unsupported iOS target: $target" >&2; exit 1 ;;
esac
cd "$root"
IPHONEOS_DEPLOYMENT_TARGET=16.0 cargo build -p gpui_mobile_demo --target "$target"
xcodegen generate --spec crates/gpui_mobile_demo/ios/project.yml
xcodebuild -project crates/gpui_mobile_demo/ios/GpuimmMobile.xcodeproj -scheme GpuimmMobile \
  -sdk "$sdk" -derivedDataPath "${CARGO_TARGET_DIR:-$root/target}/mobile-ios" \
  "GPUIMM_LIBRARY_DIR=${CARGO_TARGET_DIR:-$root/target}/$target/debug" "ARCHS=$arch" CODE_SIGNING_ALLOWED=NO "$@"
