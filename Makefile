# Entry points for Glimm development. Each target wraps a command documented in GLIMM.md.
SHELL := /bin/bash
ifeq ($(shell uname -s),Darwin)
# Keep the linker and SDK from the same Xcode installation, as run-glimm.sh does.
SDK := SDKROOT="$(shell xcrun --sdk macosx --show-sdk-path)"
endif
DEMO ?= 0
TIME ?= 1200
OUT ?= artifacts/frame.png

.PHONY: help run dev test test-wgpu check-windows check-upstream verify capture

help: ## list the targets
	@grep -E '^[a-z-]+:.*## ' $(MAKEFILE_LIST) | sed -E 's/:.*## /\t/' | expand -t 16

run: ## open the playground (ARGS="--demo 3")
	./run-glimm.sh $(ARGS)

dev: ## open the playground following the shader source: saving it recompiles
	./run-glimm.sh --dev $(ARGS)

test: ## core, Metal and playground tests
	$(SDK) cargo test -p gpui -p gpui_apple --features gpui_apple/test-support glimm --lib
	$(SDK) cargo test -p gpui_perf --example glimm --features gpui_platform_fast/test-support

test-wgpu: ## WGPU renderer tests
	$(SDK) cargo test -p gpui_wgpu --features test-support --lib fast::glimm

check-windows: ## type-check the Direct3D renderer and its tests (does not compile the HLSL)
	cargo check -p gpui_windows --features test-support --tests \
		--target x86_64-pc-windows-gnu --target-dir target/glimm-windows

check-upstream: ## upstream files may only carry hooks
	script/check-upstream

verify: test test-wgpu check-windows check-upstream ## everything that runs on this machine

capture: ## one frame: make capture DEMO=0 TIME=3290 VIEW=mix OUT=artifacts/mix.png
	@mkdir -p $(dir $(abspath $(OUT)))
	./run-glimm.sh --scene-only --demo $(DEMO) --time $(TIME) \
		$(if $(VIEW),--debug-view $(VIEW)) --capture $(abspath $(OUT))
