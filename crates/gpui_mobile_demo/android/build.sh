#!/usr/bin/env bash
set -euo pipefail
root="$(cd "$(dirname "$0")/../../.." && pwd)"
sdk="${ANDROID_HOME:-$HOME/Library/Android/sdk}"
ndk="${ANDROID_NDK_HOME:-$sdk/ndk/27.3.13750724}"
llvm="$ndk/toolchains/llvm/prebuilt/darwin-x86_64/bin"
build_tools="$sdk/build-tools/36.0.0"
android_jar="$sdk/platforms/android-36/android.jar"
output="${CARGO_TARGET_DIR:-$root/target}/mobile-android"
mkdir -p "$output/classes" "$output/dex" "$output/apk/lib/arm64-v8a"
cd "$root"
export CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER="$llvm/aarch64-linux-android26-clang"
export CC_aarch64_linux_android="$CARGO_TARGET_AARCH64_LINUX_ANDROID_LINKER"
export AR_aarch64_linux_android="$llvm/llvm-ar"
cargo build -p gpui_mobile_demo --target aarch64-linux-android
cp "${CARGO_TARGET_DIR:-$root/target}/aarch64-linux-android/debug/libgpui_mobile_demo.so" "$output/apk/lib/arm64-v8a/"
sources=()
while IFS= read -r source; do sources+=("$source"); done < <(find crates/gpui_mobile_demo/android/src -name '*.java')
javac --release 8 -classpath "$android_jar" -d "$output/classes" "${sources[@]}"
jar cf "$output/classes.jar" -C "$output/classes" .
"$build_tools/d8" --min-api 26 --lib "$android_jar" --output "$output/dex" "$output/classes.jar"
"$build_tools/aapt2" link -I "$android_jar" --manifest crates/gpui_mobile_demo/android/AndroidManifest.xml -o "$output/unsigned.apk"
cp "$output/dex/classes.dex" "$output/apk/"
(cd "$output/apk" && zip -q -r "$output/unsigned.apk" classes.dex lib)
"$build_tools/zipalign" -f -P 16 4 "$output/unsigned.apk" "$output/aligned.apk"
if [[ ! -f "$output/debug.keystore" ]]; then
    keytool -genkeypair -keystore "$output/debug.keystore" -storepass android -keypass android -alias androiddebugkey \
        -dname 'CN=Local gpuimm demo' -keyalg RSA -keysize 2048 -validity 3650
fi
"$build_tools/apksigner" sign --ks "$output/debug.keystore" --ks-pass pass:android --out "$output/gpuimm.apk" "$output/aligned.apk"
printf 'Built %s\n' "$output/gpuimm.apk"
