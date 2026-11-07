use anyhow::{Context as _, Result};
use gpui::Scene;

use crate::WgpuRenderer;

impl WgpuRenderer {
    /// Reads the native scene with this window's device, pipelines and sprite atlas.
    pub fn render_to_image(&mut self, scene: &Scene) -> Result<image::RgbaImage> {
        anyhow::ensure!(!self.device_lost(), "cannot capture a lost WGPU device");
        let size = self.viewport_size();
        let alpha = self.surface_config.alpha_mode == wgpu::CompositeAlphaMode::PreMultiplied;
        let core = self.core_mut().context("WGPU renderer is released")?;
        let width = u32::try_from(size.width.0)?;
        let height = u32::try_from(size.height.0)?;
        anyhow::ensure!(width > 0 && height > 0, "cannot capture an empty viewport");
        let device = &core.resources.device;
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("window_capture"),
            size: wgpu::Extent3d {
                width,
                height,
                depth_or_array_layers: 1,
            },
            mip_level_count: 1,
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format: core.target_format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT | wgpu::TextureUsages::COPY_SRC,
            view_formats: &[],
        });
        let row_bytes = width.checked_mul(4).context("capture row size overflow")?;
        let stride = row_bytes
            .checked_next_multiple_of(wgpu::COPY_BYTES_PER_ROW_ALIGNMENT)
            .context("capture stride overflow")?;
        let length = u64::from(stride) * u64::from(height);
        anyhow::ensure!(
            length <= device.limits().max_buffer_size,
            "capture exceeds device buffer limit"
        );
        let buffer = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("window_capture_readback"),
            size: length,
            usage: wgpu::BufferUsages::COPY_DST | wgpu::BufferUsages::MAP_READ,
            mapped_at_creation: false,
        });
        let view = texture.create_view(&Default::default());
        core.render_frame(scene, &view, size, alpha, wgpu::Color::TRANSPARENT)?;
        let device = &core.resources.device;
        let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
            label: Some("window_capture_copy"),
        });
        encoder.copy_texture_to_buffer(
            texture.as_image_copy(),
            wgpu::TexelCopyBufferInfo {
                buffer: &buffer,
                layout: wgpu::TexelCopyBufferLayout {
                    offset: 0,
                    bytes_per_row: Some(stride),
                    rows_per_image: Some(height),
                },
            },
            texture.size(),
        );
        let submission = core.resources.queue.submit([encoder.finish()]);
        let (sender, receiver) = std::sync::mpsc::channel();
        buffer
            .slice(..)
            .map_async(wgpu::MapMode::Read, move |result| {
                let _ = sender.send(result);
            });
        device.poll(wgpu::PollType::Wait {
            submission_index: Some(submission),
            timeout: Some(std::time::Duration::from_secs(30)),
        })?;
        receiver.recv_timeout(std::time::Duration::from_secs(30))??;
        let mapped = buffer.slice(..).get_mapped_range()?;
        let mut pixels = Vec::new();
        for row in mapped.chunks_exact(stride as usize).take(height as usize) {
            pixels.extend_from_slice(&row[..row_bytes as usize]);
        }
        drop(mapped);
        buffer.unmap();
        if matches!(
            core.target_format,
            wgpu::TextureFormat::Bgra8Unorm | wgpu::TextureFormat::Bgra8UnormSrgb
        ) {
            for pixel in pixels.chunks_exact_mut(4) {
                pixel.swap(0, 2);
            }
        }
        image::RgbaImage::from_raw(width, height, pixels)
            .context("invalid capture image dimensions")
    }
}
