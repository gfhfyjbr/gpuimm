use anyhow::Result;
use gpui::{
    AtlasTextureId, AtlasTextureKind, AtlasTile, Bounds, ContentMask, Corners, DevicePixels,
    GlimmFrame, GlimmUniforms, PolychromeSprite, ScaledPixels, Scene, Size, TileId, point, size,
};
use wgpu::util::DeviceExt as _;

use crate::wgpu_renderer::{WgpuPipelines, WgpuRendererCore};

const TEXTURE_BASE: u32 = 0xd000_0000;

#[derive(Default)]
pub(crate) struct GlimmRenderer {
    rendering: bool,
    pipeline: Option<Pipeline>,
    raster_pipelines: Option<WgpuPipelines>,
    live: Option<Texture>,
    raster: Option<Texture>,
    old: Vec<(u64, Texture)>,
    current: Vec<(u64, Texture)>,
    outputs: Vec<(u64, Texture)>,
    pub(crate) images: Vec<wgpu::TextureView>,
    viewport: [u32; 2],
}

struct Pipeline {
    layout: wgpu::BindGroupLayout,
    paint: wgpu::RenderPipeline,
    mipmap: wgpu::RenderPipeline,
    sampler: wgpu::Sampler,
    empty_uniforms: wgpu::Buffer,
}

#[derive(Clone)]
struct Texture {
    texture: wgpu::Texture,
    view: wgpu::TextureView,
    size: [u32; 2],
}

impl Texture {
    fn new(
        device: &wgpu::Device,
        format: wgpu::TextureFormat,
        size: [u32; 2],
        mipped: bool,
    ) -> Self {
        let texture = device.create_texture(&wgpu::TextureDescriptor {
            label: Some("glimm_texture"),
            size: wgpu::Extent3d {
                width: size[0],
                height: size[1],
                depth_or_array_layers: 1,
            },
            mip_level_count: if mipped {
                size[0].max(size[1]).ilog2() + 1
            } else {
                1
            },
            sample_count: 1,
            dimension: wgpu::TextureDimension::D2,
            format,
            usage: wgpu::TextureUsages::RENDER_ATTACHMENT
                | wgpu::TextureUsages::TEXTURE_BINDING
                | wgpu::TextureUsages::COPY_SRC
                | wgpu::TextureUsages::COPY_DST,
            view_formats: &[],
        });
        let view = texture.create_view(&Default::default());
        Self {
            texture,
            view,
            size,
        }
    }
}

impl Pipeline {
    fn new(device: &wgpu::Device, format: wgpu::TextureFormat) -> Self {
        let shader = device.create_shader_module(wgpu::ShaderModuleDescriptor {
            label: Some("glimm_shader"),
            source: wgpu::ShaderSource::Wgsl(include_str!("glimm.wgsl").into()),
        });
        let layout = device.create_bind_group_layout(&wgpu::BindGroupLayoutDescriptor {
            label: Some("glimm_bindings"),
            entries: &[
                wgpu::BindGroupLayoutEntry {
                    binding: 0,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Buffer {
                        ty: wgpu::BufferBindingType::Uniform,
                        has_dynamic_offset: false,
                        min_binding_size: std::num::NonZeroU64::new(
                            size_of::<GlimmUniforms>() as u64
                        ),
                    },
                    count: None,
                },
                texture_entry(1),
                texture_entry(2),
                wgpu::BindGroupLayoutEntry {
                    binding: 3,
                    visibility: wgpu::ShaderStages::FRAGMENT,
                    ty: wgpu::BindingType::Sampler(wgpu::SamplerBindingType::Filtering),
                    count: None,
                },
            ],
        });
        let pipeline_layout = device.create_pipeline_layout(&wgpu::PipelineLayoutDescriptor {
            label: Some("glimm_pipeline_layout"),
            bind_group_layouts: &[Some(&layout)],
            immediate_size: 0,
        });
        let create_pipeline = |entry| {
            device.create_render_pipeline(&wgpu::RenderPipelineDescriptor {
                label: Some(entry),
                layout: Some(&pipeline_layout),
                vertex: wgpu::VertexState {
                    module: &shader,
                    entry_point: Some("glimm_vertex"),
                    buffers: &[],
                    compilation_options: Default::default(),
                },
                fragment: Some(wgpu::FragmentState {
                    module: &shader,
                    entry_point: Some(entry),
                    targets: &[Some(wgpu::ColorTargetState {
                        format,
                        blend: None,
                        write_mask: wgpu::ColorWrites::ALL,
                    })],
                    compilation_options: Default::default(),
                }),
                primitive: Default::default(),
                depth_stencil: None,
                multisample: Default::default(),
                multiview_mask: None,
                cache: None,
            })
        };
        let paint = create_pipeline("glimm_fragment");
        let mipmap = create_pipeline("mipmap_fragment");
        let sampler = device.create_sampler(&wgpu::SamplerDescriptor {
            label: Some("glimm_trilinear"),
            mag_filter: wgpu::FilterMode::Linear,
            min_filter: wgpu::FilterMode::Linear,
            mipmap_filter: wgpu::MipmapFilterMode::Linear,
            ..Default::default()
        });
        let empty_uniforms = device.create_buffer(&wgpu::BufferDescriptor {
            label: Some("glimm_mipmap_uniforms"),
            size: size_of::<GlimmUniforms>() as u64,
            usage: wgpu::BufferUsages::UNIFORM,
            mapped_at_creation: false,
        });
        Self {
            layout,
            paint,
            mipmap,
            sampler,
            empty_uniforms,
        }
    }

    fn bindings(
        &self,
        device: &wgpu::Device,
        uniforms: &wgpu::Buffer,
        live: &wgpu::TextureView,
        old: &wgpu::TextureView,
    ) -> wgpu::BindGroup {
        device.create_bind_group(&wgpu::BindGroupDescriptor {
            label: Some("glimm_bind_group"),
            layout: &self.layout,
            entries: &[
                wgpu::BindGroupEntry {
                    binding: 0,
                    resource: uniforms.as_entire_binding(),
                },
                wgpu::BindGroupEntry {
                    binding: 1,
                    resource: wgpu::BindingResource::TextureView(live),
                },
                wgpu::BindGroupEntry {
                    binding: 2,
                    resource: wgpu::BindingResource::TextureView(old),
                },
                wgpu::BindGroupEntry {
                    binding: 3,
                    resource: wgpu::BindingResource::Sampler(&self.sampler),
                },
            ],
        })
    }

    fn generate_mipmaps(
        &self,
        device: &wgpu::Device,
        encoder: &mut wgpu::CommandEncoder,
        texture: &Texture,
    ) {
        for mip in 1..texture.texture.mip_level_count() {
            let source = texture.texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: mip - 1,
                mip_level_count: Some(1),
                ..Default::default()
            });
            let destination = texture.texture.create_view(&wgpu::TextureViewDescriptor {
                base_mip_level: mip,
                mip_level_count: Some(1),
                ..Default::default()
            });
            let bindings = self.bindings(device, &self.empty_uniforms, &source, &source);
            let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
                label: Some("glimm_mipmap"),
                color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                    view: &destination,
                    depth_slice: None,
                    resolve_target: None,
                    ops: wgpu::Operations {
                        load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                        store: wgpu::StoreOp::Store,
                    },
                })],
                ..Default::default()
            });
            pass.set_pipeline(&self.mipmap);
            pass.set_bind_group(0, &bindings, &[]);
            pass.draw(0..3, 0..1);
        }
    }
}

fn texture_entry(binding: u32) -> wgpu::BindGroupLayoutEntry {
    wgpu::BindGroupLayoutEntry {
        binding,
        visibility: wgpu::ShaderStages::FRAGMENT,
        ty: wgpu::BindingType::Texture {
            sample_type: wgpu::TextureSampleType::Float { filterable: true },
            view_dimension: wgpu::TextureViewDimension::D2,
            multisampled: false,
        },
        count: None,
    }
}

pub(crate) fn image(
    images: &[wgpu::TextureView],
    id: AtlasTextureId,
) -> Option<&wgpu::TextureView> {
    if !is_texture(id) {
        return None;
    }
    images.get((id.index & 0x0fff_ffff) as usize)
}

pub(crate) fn is_texture(id: AtlasTextureId) -> bool {
    id.kind == AtlasTextureKind::Polychrome && id.index & 0xf000_0000 == TEXTURE_BASE
}

impl GlimmRenderer {
    fn clear(&mut self) {
        self.live = None;
        self.raster = None;
        self.old.clear();
        self.current.clear();
        self.outputs.clear();
        self.images.clear();
    }
}

/// Composes each scope at its original paint position; recursive raster draws bypass this hook.
pub(crate) fn render(
    renderer: &mut WgpuRendererCore,
    scene: &Scene,
    target: &wgpu::TextureView,
    viewport: Size<DevicePixels>,
    premultiplied_alpha: bool,
    clear_color: wgpu::Color,
) -> Result<Option<wgpu::SubmissionIndex>> {
    if renderer.fast_glimm.rendering {
        return Ok(None);
    }
    if scene.fast_glimm.frames.is_empty() {
        renderer.fast_glimm.clear();
        return Ok(None);
    }
    anyhow::ensure!(
        viewport.width.0 > 0 && viewport.height.0 > 0,
        "invalid Glimm viewport"
    );
    let dimensions = [viewport.width.0 as u32, viewport.height.0 as u32];
    anyhow::ensure!(
        dimensions.iter().all(|n| *n
            <= renderer
                .resources()
                .device
                .limits()
                .max_texture_dimension_2d),
        "Glimm viewport exceeds the device limit"
    );
    let mut state = std::mem::take(&mut renderer.fast_glimm);
    renderer.fast_glimm.rendering = true;
    let result = (|| {
        if state.viewport != dimensions {
            state.clear();
            state.viewport = dimensions;
        }
        if state.pipeline.is_none() {
            state.pipeline = Some(Pipeline::new(
                &renderer.resources().device,
                renderer.target_format,
            ));
            let target = crate::fast::frame::FrameHost::target(renderer)?;
            state.raster_pipelines = Some(WgpuRendererCore::create_pipelines(
                target.device,
                target.bind_group_layouts,
                target.format,
                wgpu::CompositeAlphaMode::PreMultiplied,
                target.path_sample_count,
                target.pipelines.subpixel_sprites.is_some(),
                matches!(
                    target.instance_data,
                    crate::fast::frame::InstanceSource::Texture(_)
                ),
            ));
        }
        state.images.clear();
        for cache in [&mut state.old, &mut state.current, &mut state.outputs] {
            cache.retain(|(id, _)| scene.fast_glimm.frames.iter().any(|f| f.run_id == *id));
        }
        let mut composed = Scene::default();
        composed.replay(0..scene.len(), scene);
        composed.fast_glimm.frames.clear();
        let mut frames = scene.fast_glimm.frames.clone();
        frames.sort_by_key(|frame| frame.paint_range.len());
        for i in 0..frames.len() {
            let frame = &frames[i];
            let range = frame.paint_range.clone();
            let mut content = Scene::default();
            content.replay_balanced(range.clone(), &composed)?;
            content.finish();
            let cached = state
                .current
                .iter()
                .find(|(id, _)| *id == frame.run_id)
                .map(|(_, texture)| texture.clone());
            let live = render_content(renderer, &mut state, &content, frame, viewport, cached)?;
            if state
                .old
                .iter()
                .any(|(id, old)| *id == frame.run_id && old.size != live.size)
            {
                state.old.retain(|(id, _)| *id != frame.run_id);
            }
            if !state.old.iter().any(|(id, _)| *id == frame.run_id) {
                let old = render_content(
                    renderer,
                    &mut state,
                    &frame.old_scene,
                    frame,
                    viewport,
                    None,
                )?;
                state.old.push((frame.run_id, old));
            }
            if let Some((_, texture)) = state.current.iter_mut().find(|(id, _)| *id == frame.run_id)
            {
                *texture = live.clone();
            } else {
                state.current.push((frame.run_id, live.clone()));
            }
            state.live = Some(live);
            if !state.outputs.iter().any(|(id, _)| *id == frame.run_id) {
                state.outputs.push((
                    frame.run_id,
                    Texture::new(
                        &renderer.resources().device,
                        renderer.target_format,
                        dimensions,
                        false,
                    ),
                ));
            }
            let live = state.live.as_ref().unwrap();
            let old = &state
                .old
                .iter()
                .find(|(id, _)| *id == frame.run_id)
                .unwrap()
                .1;
            let output = &state
                .outputs
                .iter()
                .find(|(id, _)| *id == frame.run_id)
                .unwrap()
                .1;
            paint(
                renderer,
                state.pipeline.as_ref().unwrap(),
                frame,
                live,
                old,
                output,
            );
            let index = renderer.fast_glimm.images.len();
            renderer.fast_glimm.images.push(output.view.clone());
            let mut next = Scene::default();
            next.replay(0..range.start, &composed);
            next.insert_primitive(sprite(index, frame.clip, dimensions));
            next.replay(range.end..composed.len(), &composed);
            next.fast_glimm.frames.clear();
            composed = next;
            for pending in &mut frames[i + 1..] {
                rebase(&mut pending.paint_range, &range)?;
            }
        }
        composed.finish();
        renderer.render_frame(
            &composed,
            target,
            viewport,
            premultiplied_alpha,
            clear_color,
        )
    })();
    state.images = std::mem::take(&mut renderer.fast_glimm.images);
    state.rendering = false;
    renderer.fast_glimm = state;
    result.map(Some)
}

fn render_content(
    renderer: &mut WgpuRendererCore,
    state: &mut GlimmRenderer,
    scene: &Scene,
    frame: &GlimmFrame,
    viewport: Size<DevicePixels>,
    cached: Option<Texture>,
) -> Result<Texture> {
    if state.raster.is_none() {
        state.raster = Some(Texture::new(
            &renderer.resources().device,
            renderer.target_format,
            state.viewport,
            false,
        ));
    }
    let raster = state.raster.as_ref().unwrap();
    // The scoped raster must accumulate premultiplied color over transparent black,
    // independently of the window's opaque/straight-alpha presentation pipeline.
    std::mem::swap(
        &mut renderer.resources_mut().pipelines,
        state.raster_pipelines.as_mut().unwrap(),
    );
    let result = renderer.render_frame(
        scene,
        &raster.view,
        viewport,
        true,
        wgpu::Color::TRANSPARENT,
    );
    std::mem::swap(
        &mut renderer.resources_mut().pipelines,
        state.raster_pipelines.as_mut().unwrap(),
    );
    result?;
    let [x, y, width, height] = crop(frame, state.viewport);
    let texture = cached
        .filter(|texture| texture.size == [width, height])
        .unwrap_or_else(|| {
            Texture::new(
                &renderer.resources().device,
                renderer.target_format,
                [width, height],
                true,
            )
        });
    let mut encoder =
        renderer
            .resources()
            .device
            .create_command_encoder(&wgpu::CommandEncoderDescriptor {
                label: Some("glimm_crop"),
            });
    encoder.copy_texture_to_texture(
        wgpu::TexelCopyTextureInfo {
            texture: &raster.texture,
            mip_level: 0,
            origin: wgpu::Origin3d { x, y, z: 0 },
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::TexelCopyTextureInfo {
            texture: &texture.texture,
            mip_level: 0,
            origin: wgpu::Origin3d::ZERO,
            aspect: wgpu::TextureAspect::All,
        },
        wgpu::Extent3d {
            width,
            height,
            depth_or_array_layers: 1,
        },
    );
    state.pipeline.as_ref().unwrap().generate_mipmaps(
        &renderer.resources().device,
        &mut encoder,
        &texture,
    );
    renderer
        .resources()
        .queue
        .submit(std::iter::once(encoder.finish()));
    Ok(texture)
}

fn crop(frame: &GlimmFrame, viewport: [u32; 2]) -> [u32; 4] {
    let x = frame.bounds.origin.x.0.floor().max(0.) as u32;
    let y = frame.bounds.origin.y.0.floor().max(0.) as u32;
    let x = x.min(viewport[0] - 1);
    let y = y.min(viewport[1] - 1);
    let right = frame.bounds.right().0.ceil().max(0.) as u32;
    let bottom = frame.bounds.bottom().0.ceil().max(0.) as u32;
    [
        x,
        y,
        right.min(viewport[0]).saturating_sub(x).max(1),
        bottom.min(viewport[1]).saturating_sub(y).max(1),
    ]
}

fn paint(
    renderer: &WgpuRendererCore,
    pipeline: &Pipeline,
    frame: &GlimmFrame,
    live: &Texture,
    old: &Texture,
    output: &Texture,
) {
    let mut uniforms = frame.uniforms;
    let [x, y, _, _] = crop(frame, output.size);
    uniforms.0[1][2] = frame.bounds.origin.x.0 - x as f32;
    uniforms.0[1][3] = frame.bounds.origin.y.0 - y as f32;
    let device = &renderer.resources().device;
    let buffer = device.create_buffer_init(&wgpu::util::BufferInitDescriptor {
        label: Some("glimm_uniforms"),
        contents: bytemuck::cast_slice(&uniforms.0),
        usage: wgpu::BufferUsages::UNIFORM,
    });
    let bindings = pipeline.bindings(device, &buffer, &live.view, &old.view);
    let mut encoder = device.create_command_encoder(&wgpu::CommandEncoderDescriptor {
        label: Some("glimm_effect"),
    });
    {
        let mut pass = encoder.begin_render_pass(&wgpu::RenderPassDescriptor {
            label: Some("glimm_effect"),
            color_attachments: &[Some(wgpu::RenderPassColorAttachment {
                view: &output.view,
                depth_slice: None,
                resolve_target: None,
                ops: wgpu::Operations {
                    load: wgpu::LoadOp::Clear(wgpu::Color::TRANSPARENT),
                    store: wgpu::StoreOp::Store,
                },
            })],
            ..Default::default()
        });
        let clip = frame.clip;
        let left = clip.origin.x.0.floor().max(0.) as u32;
        let top = clip.origin.y.0.floor().max(0.) as u32;
        let right = (clip.right().0.ceil().max(0.) as u32).min(output.size[0]);
        let bottom = (clip.bottom().0.ceil().max(0.) as u32).min(output.size[1]);
        if right > left && bottom > top {
            pass.set_scissor_rect(left, top, right - left, bottom - top);
            pass.set_pipeline(&pipeline.paint);
            pass.set_bind_group(0, &bindings, &[]);
            pass.draw(0..3, 0..1);
        }
    }
    renderer
        .resources()
        .queue
        .submit(std::iter::once(encoder.finish()));
}

fn rebase(pending: &mut std::ops::Range<usize>, replaced: &std::ops::Range<usize>) -> Result<()> {
    let shift = |n: usize| n - replaced.len() + 1;
    if pending.start >= replaced.end
        && !(pending.start == replaced.start && pending.end == replaced.end)
    {
        pending.start = shift(pending.start);
        pending.end = shift(pending.end);
    } else if pending.start <= replaced.start && pending.end >= replaced.end {
        pending.end = shift(pending.end);
    } else {
        anyhow::ensure!(pending.end <= replaced.start, "crossing glimm paint ranges");
    }
    Ok(())
}

fn sprite(index: usize, clip: Bounds<ScaledPixels>, viewport: [u32; 2]) -> PolychromeSprite {
    PolychromeSprite {
        order: 0,
        pad: 0,
        grayscale: false.into(),
        opacity: 1.,
        bounds: Bounds::new(
            point(ScaledPixels(0.), ScaledPixels(0.)),
            size(
                ScaledPixels(viewport[0] as f32),
                ScaledPixels(viewport[1] as f32),
            ),
        ),
        content_mask: ContentMask { bounds: clip },
        corner_radii: Corners::default(),
        tile: AtlasTile {
            texture_id: AtlasTextureId {
                index: TEXTURE_BASE | index as u32,
                kind: AtlasTextureKind::Polychrome,
            },
            tile_id: TileId(0),
            padding: 0,
            bounds: Bounds::new(
                point(DevicePixels(0), DevicePixels(0)),
                size(
                    DevicePixels(viewport[0] as i32),
                    DevicePixels(viewport[1] as i32),
                ),
            ),
        },
    }
}

#[cfg(all(test, not(target_family = "wasm")))]
mod tests;
