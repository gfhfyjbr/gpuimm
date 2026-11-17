#ifndef GPUIMM_H
#define GPUIMM_H
#include <stdbool.h>
#include <stddef.h>
#include <stdint.h>
// All calls belong to the creating UI thread; never call reentrantly.
// Keep the CAMetalLayer-backed UIView alive until gpuimm_destroy returns.
uint64_t gpuimm_create_ios(void *view, uint32_t width, uint32_t height, float scale);
int32_t gpuimm_destroy(uint64_t handle);
int32_t gpuimm_tick(uint64_t handle);
int32_t gpuimm_resize(uint64_t handle, uint32_t width, uint32_t height, float scale);
int32_t gpuimm_lifecycle(uint64_t handle, bool active);
int32_t gpuimm_appearance(uint64_t handle, bool dark, bool reduce_motion);
int32_t gpuimm_memory_warning(uint64_t handle);
int32_t gpuimm_advance(uint64_t handle);
int32_t gpuimm_count(uint64_t handle);
int32_t gpuimm_set_title(uint64_t handle, const uint8_t *bytes, size_t length);
// Coordinates are view-local points/dp; phase: began=0, moved=1, ended=2, cancelled=3.
int32_t gpuimm_touch(uint64_t handle, uint64_t id, uint32_t phase, float x, float y);
#endif
