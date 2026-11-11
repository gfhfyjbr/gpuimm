#!/bin/bash
set -euo pipefail
sudo mkdir -p /workspace /host-cargo
mountpoint -q /workspace || sudo mount -t virtiofs glimm /workspace
mountpoint -q /host-cargo || sudo mount -t virtiofs glimm-cargo /host-cargo
if [[ ! -f /workspace/Cargo.toml && -f /workspace/workspace/Cargo.toml ]]; then
  sudo mount --bind /workspace/workspace /workspace
fi
if [[ ! -d /host-cargo/registry && -d /host-cargo/cargo/registry ]]; then
  sudo mount --bind /host-cargo/cargo /host-cargo
fi
if [[ -n "${GLIMM_HTTP_PROXY:-}" ]]; then
  export http_proxy="$GLIMM_HTTP_PROXY" https_proxy="$GLIMM_HTTP_PROXY"
  export HTTP_PROXY="$GLIMM_HTTP_PROXY" HTTPS_PROXY="$GLIMM_HTTP_PROXY"
fi
apt() { sudo env http_proxy="${http_proxy:-}" https_proxy="${https_proxy:-}" apt-get "$@"; }
apt update
apt install -y --no-install-recommends \
  curl ca-certificates pkg-config clang lld libfontconfig-dev libfreetype-dev \
  libssl-dev libzstd-dev libwayland-dev libx11-xcb-dev libxcb1-dev \
  libxcb-shape0-dev libxcb-xfixes0-dev libxrandr-dev libxkbcommon-dev \
  libxkbcommon-x11-dev libasound2-dev libvulkan1 mesa-vulkan-drivers \
  vulkan-tools libegl1 libgles2 libgtk-3-dev libwebkit2gtk-4.1-dev \
  xvfb xauth fonts-dejavu-core nodejs
if [[ ! -x "$HOME/.cargo/bin/rustup" ]]; then
  curl --proto '=https' --tlsv1.2 -fsSLo /tmp/glimm-rustup-init.sh https://sh.rustup.rs
  sh /tmp/glimm-rustup-init.sh -y --profile minimal --default-toolchain 1.98.1
fi
export PATH="$HOME/.cargo/bin:$PATH"
rustup toolchain install 1.98.1 --profile minimal --component clippy --component rustfmt
for cache in registry git; do
  if [[ ! -e "$HOME/.cargo/$cache" ]]; then
    ln -s "/host-cargo/$cache" "$HOME/.cargo/$cache"
  fi
done
export CARGO_TARGET_DIR=/workspace/target/glimm-linux
export CARGO_INCREMENTAL=0 CARGO_PROFILE_DEV_DEBUG=0 CARGO_BUILD_JOBS=4
export GLIMM_ARTIFACTS=/workspace
export XDG_RUNTIME_DIR=/tmp/glimm-runtime
mkdir -p "$XDG_RUNTIME_DIR"
chmod 700 "$XDG_RUNTIME_DIR"
uname -a
rustc --version
vulkaninfo --summary
cd /workspace
cargo test -p gpui_wgpu --features test-support --lib -- --nocapture
