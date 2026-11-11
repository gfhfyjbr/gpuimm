use crate::metal_renderer::{MetalRenderer, new_command_encoder_for_texture};
use anyhow::{Context as _, Result};
use gpui::{
    AtlasTextureId, AtlasTextureKind, AtlasTile, Bounds, ContentMask, Corners, DevicePixels,
    GlimmFrame, GlimmShaderSource, GlimmUniforms, PolychromeSprite, ScaledPixels, Scene, Size,
    TileId, point, size,
};
use objc2::{rc::Retained, runtime::ProtocolObject};
use objc2_foundation::NSString;
use objc2_metal::{
    MTLBlitCommandEncoder, MTLClearColor, MTLCommandBuffer, MTLCommandEncoder, MTLDevice,
    MTLLibrary, MTLOrigin, MTLPixelFormat, MTLPrimitiveType, MTLRenderCommandEncoder,
    MTLRenderPipelineDescriptor, MTLRenderPipelineState, MTLScissorRect, MTLSize, MTLStorageMode,
    MTLTexture, MTLTextureDescriptor, MTLTextureUsage,
};
use std::{collections::HashMap, mem, ops::Range, ptr::NonNull};

type Texture = Retained<ProtocolObject<dyn MTLTexture>>;
// Separate from dynamic textures (0xE...) and retained scroll tiles (0xF...).
const TEXTURE_BASE: u32 = 0xD000_0000;
#[derive(Default)]
pub(crate) struct GlimmRenderer {
    rendering: bool,
    source: Option<GlimmShaderSource>,
    pipeline: Option<Retained<ProtocolObject<dyn MTLRenderPipelineState>>>,
    live: Option<Texture>,
    old: HashMap<u64, Texture>,
    current: HashMap<u64, Texture>,
    outputs: HashMap<u64, Texture>,
    images: Vec<Texture>,
}

pub(crate) fn image(state: &GlimmRenderer, id: AtlasTextureId) -> Option<Texture> {
    (id.kind == AtlasTextureKind::Polychrome)
        .then(|| id.index.checked_sub(TEXTURE_BASE))
        .flatten()
        .and_then(|index| state.images.get(index as usize))
        .cloned()
}

pub(crate) fn render(
    renderer: &mut MetalRenderer,
    scene: &Scene,
    target: &ProtocolObject<dyn MTLTexture>,
    viewport: Size<DevicePixels>,
) -> Result<Option<Retained<ProtocolObject<dyn MTLCommandBuffer>>>> {
    if renderer.fast_glimm.rendering {
        return Ok(None);
    }
    if scene.fast_glimm.frames.is_empty() {
        renderer.fast_glimm.live = None;
        renderer.fast_glimm.old.clear();
        renderer.fast_glimm.current.clear();
        renderer.fast_glimm.outputs.clear();
        renderer.fast_glimm.images.clear();
        return Ok(None);
    }
    let mut state = mem::take(&mut renderer.fast_glimm);
    renderer.fast_glimm.rendering = true;
    let result = render_inner(renderer, &mut state, scene, target, viewport);
    state.images = mem::take(&mut renderer.fast_glimm.images);
    renderer.fast_glimm = state;
    result.map(Some)
}

fn render_inner(
    renderer: &mut MetalRenderer,
    state: &mut GlimmRenderer,
    scene: &Scene,
    target: &ProtocolObject<dyn MTLTexture>,
    viewport: Size<DevicePixels>,
) -> Result<Retained<ProtocolObject<dyn MTLCommandBuffer>>> {
    let w = viewport.width.0 as usize;
    let h = viewport.height.0 as usize;
    let source = state
        .source
        .get_or_insert_with(|| GlimmShaderSource::new("glimm.metal", include_str!("glimm.metal")));
    // A followed shader that fails to build keeps the previous pipeline in use.
    if let Some(text) = source.take_changed() {
        match pipeline(&renderer.device, target.pixelFormat(), &text) {
            Ok(pipeline) => {
                state.pipeline = Some(pipeline);
                gpui::glimm_report_shader(Ok(()));
            }
            Err(error) => gpui::glimm_report_shader(Err(format!("{error:#}"))),
        }
    }
    if state.pipeline.is_none() {
        state.pipeline = Some(pipeline(
            &renderer.device,
            target.pixelFormat(),
            source.embedded(),
        )?);
    }
    if state
        .live
        .as_ref()
        .is_none_or(|t| t.width() != w || t.height() != h)
    {
        state.live = Some(texture(
            &renderer.device,
            w,
            h,
            target.pixelFormat(),
            false,
        )?);
        state.old.clear();
        state.current.clear();
        state.outputs.clear();
    }
    state
        .old
        .retain(|id, _| scene.fast_glimm.frames.iter().any(|f| f.run_id == *id));
    state
        .outputs
        .retain(|id, _| scene.fast_glimm.frames.iter().any(|f| f.run_id == *id));
    state
        .current
        .retain(|id, _| scene.fast_glimm.frames.iter().any(|f| f.run_id == *id));
    let mut composed = Scene::default();
    composed.replay(0..scene.len(), scene);
    composed.fast_glimm.frames.clear();
    let mut frames = scene.fast_glimm.frames.clone();
    // Inner scopes are rasterized first. The parent then samples their composited pixels.
    frames.sort_by_key(|f| f.paint_range.len());
    for index in 0..frames.len() {
        let frame = &frames[index];
        let range = frame.paint_range.clone();
        let mut content = Scene::default();
        content.replay_balanced(range.clone(), &composed)?;
        content.finish();
        let crop_x = (frame.bounds.origin.x.0.floor().max(0.) as usize).min(w - 1);
        let crop_y = (frame.bounds.origin.y.0.floor().max(0.) as usize).min(h - 1);
        let crop_w = ((frame.bounds.right().0.ceil().max(0.) as usize).min(w) - crop_x).max(1);
        let crop_h = ((frame.bounds.bottom().0.ceil().max(0.) as usize).min(h) - crop_y).max(1);
        let raster = state.live.as_ref().context("glimm raster target")?;
        if state
            .current
            .get(&frame.run_id)
            .is_some_and(|t| t.width() != crop_w || t.height() != crop_h)
        {
            state.current.remove(&frame.run_id);
            state.old.remove(&frame.run_id);
        }
        if let std::collections::hash_map::Entry::Vacant(entry) = state.old.entry(frame.run_id) {
            let old = texture(&renderer.device, crop_w, crop_h, target.pixelFormat(), true)?;
            let buffer = render_content(renderer, &frame.old_scene, raster, viewport)?;
            crop(&buffer, raster, &old, crop_x, crop_y)?;
            buffer.commit();
            entry.insert(old);
        }
        if let std::collections::hash_map::Entry::Vacant(entry) = state.current.entry(frame.run_id)
        {
            entry.insert(texture(
                &renderer.device,
                crop_w,
                crop_h,
                target.pixelFormat(),
                true,
            )?);
        }
        if let std::collections::hash_map::Entry::Vacant(entry) = state.outputs.entry(frame.run_id)
        {
            entry.insert(texture(
                &renderer.device,
                w,
                h,
                target.pixelFormat(),
                false,
            )?);
        }
        let live = state
            .current
            .get(&frame.run_id)
            .context("glimm live crop")?;
        let old = state.old.get(&frame.run_id).context("glimm old texture")?;
        let output = state
            .outputs
            .get(&frame.run_id)
            .context("glimm output texture")?;
        let buffer = render_content(renderer, &content, raster, viewport)?;
        crop(&buffer, raster, live, crop_x, crop_y)?;
        let encoder = new_command_encoder_for_texture(
            &buffer,
            output,
            viewport,
            Some(MTLClearColor {
                red: 0.,
                green: 0.,
                blue: 0.,
                alpha: 0.,
            }),
        )?;
        encoder.setLabel(Some(&NSString::from_str(
            "glimm: isolated native content + iridescent band",
        )));
        let x = (frame.clip.origin.x.0.floor().max(0.) as usize).min(w);
        let y = (frame.clip.origin.y.0.floor().max(0.) as usize).min(h);
        let right = (frame.clip.right().0.ceil().max(0.) as usize).min(w);
        let bottom = (frame.clip.bottom().0.ceil().max(0.) as usize).min(h);
        if right > x && bottom > y {
            let mut uniforms: GlimmUniforms = frame.uniforms;
            uniforms.0[1][2] = frame.bounds.origin.x.0 - crop_x as f32;
            uniforms.0[1][3] = frame.bounds.origin.y.0 - crop_y as f32;
            encoder.setRenderPipelineState(state.pipeline.as_ref().context("glimm pipeline")?);
            unsafe {
                // C-layout float4 rows are fully initialized and copied by Metal.
                encoder.setFragmentBytes_length_atIndex(
                    NonNull::from(&uniforms).cast(),
                    mem::size_of::<GlimmUniforms>(),
                    0,
                );
                encoder.setFragmentTexture_atIndex(Some(live), 0);
                encoder.setFragmentTexture_atIndex(Some(old), 1);
                encoder.setScissorRect(MTLScissorRect {
                    x,
                    y,
                    width: right - x,
                    height: bottom - y,
                });
                encoder.drawPrimitives_vertexStart_vertexCount(MTLPrimitiveType::Triangle, 0, 3);
            }
        }
        encoder.endEncoding();
        buffer.commit();
        let image_index = renderer.fast_glimm.images.len();
        renderer.fast_glimm.images.push(output.clone());
        let sprite = sprite(frame, image_index, viewport);
        let mut next = Scene::default();
        next.replay(0..range.start, &composed);
        next.insert_primitive(sprite);
        next.replay(range.end..composed.len(), &composed);
        next.fast_glimm.frames.clear();
        composed = next;
        for remaining in &mut frames[index + 1..] {
            rebase(&mut remaining.paint_range, &range)?;
        }
    }
    composed.finish();
    renderer.render_frame(&composed, target, viewport)
}
fn render_content(
    renderer: &mut MetalRenderer,
    scene: &Scene,
    target: &ProtocolObject<dyn MTLTexture>,
    viewport: Size<DevicePixels>,
) -> Result<Retained<ProtocolObject<dyn MTLCommandBuffer>>> {
    let opaque = renderer.opaque;
    renderer.opaque = false;
    let result = renderer.render_frame(scene, target, viewport);
    renderer.opaque = opaque;
    result
}
fn rebase(pending: &mut Range<usize>, replaced: &Range<usize>) -> Result<()> {
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
fn sprite(frame: &GlimmFrame, index: usize, viewport: Size<DevicePixels>) -> PolychromeSprite {
    PolychromeSprite {
        order: 0,
        pad: 0,
        grayscale: false.into(),
        opacity: 1.,
        bounds: Bounds::new(
            point(ScaledPixels(0.), ScaledPixels(0.)),
            size(
                ScaledPixels(viewport.width.0 as f32),
                ScaledPixels(viewport.height.0 as f32),
            ),
        ),
        content_mask: ContentMask { bounds: frame.clip },
        corner_radii: Corners::default(),
        tile: AtlasTile {
            texture_id: AtlasTextureId {
                kind: AtlasTextureKind::Polychrome,
                index: TEXTURE_BASE + index as u32,
            },
            tile_id: TileId(0),
            padding: 0,
            bounds: Bounds::new(point(DevicePixels(0), DevicePixels(0)), viewport),
        },
    }
}
fn texture(
    device: &ProtocolObject<dyn MTLDevice>,
    w: usize,
    h: usize,
    format: MTLPixelFormat,
    mipmapped: bool,
) -> Result<Texture> {
    let descriptor = MTLTextureDescriptor::new();
    unsafe {
        descriptor.setWidth(w);
        descriptor.setHeight(h);
        descriptor.setMipmapLevelCount(if mipmapped {
            w.max(h).ilog2() as usize + 1
        } else {
            1
        });
    }
    descriptor.setPixelFormat(format);
    descriptor.setStorageMode(MTLStorageMode::Private);
    descriptor.setUsage(MTLTextureUsage::RenderTarget | MTLTextureUsage::ShaderRead);
    device
        .newTextureWithDescriptor(&descriptor)
        .context("allocating glimm render target")
}
fn crop(
    buffer: &ProtocolObject<dyn MTLCommandBuffer>,
    source: &ProtocolObject<dyn MTLTexture>,
    target: &ProtocolObject<dyn MTLTexture>,
    x: usize,
    y: usize,
) -> Result<()> {
    let encoder = buffer
        .blitCommandEncoder()
        .context("creating glimm crop encoder")?;
    // The source rectangle is clamped to the viewport and matches the destination extent.
    unsafe {
        encoder.copyFromTexture_sourceSlice_sourceLevel_sourceOrigin_sourceSize_toTexture_destinationSlice_destinationLevel_destinationOrigin(
            source,0,0,MTLOrigin{x,y,z:0},MTLSize{width:target.width(),height:target.height(),depth:1},
            target,0,0,MTLOrigin{x:0,y:0,z:0},
        );
    }
    encoder.endEncoding();
    mipmaps(buffer, target)
}
fn mipmaps(
    buffer: &ProtocolObject<dyn MTLCommandBuffer>,
    texture: &ProtocolObject<dyn MTLTexture>,
) -> Result<()> {
    if texture.mipmapLevelCount() < 2 {
        return Ok(());
    }
    let encoder = buffer
        .blitCommandEncoder()
        .context("creating glimm mipmap encoder")?;
    encoder.generateMipmapsForTexture(texture);
    encoder.endEncoding();
    Ok(())
}
fn pipeline(
    device: &ProtocolObject<dyn MTLDevice>,
    format: MTLPixelFormat,
    source: &str,
) -> Result<Retained<ProtocolObject<dyn MTLRenderPipelineState>>> {
    let library = device
        .newLibraryWithSource_options_error(&NSString::from_str(source), None)
        .map_err(|error| anyhow::anyhow!("{}", error.localizedDescription()))
        .context("compiling glimm Metal shaders")?;
    let descriptor = MTLRenderPipelineDescriptor::new();
    let vertex = library
        .newFunctionWithName(&NSString::from_str("glimm_vertex"))
        .context("glimm vertex function")?;
    let fragment = library
        .newFunctionWithName(&NSString::from_str("glimm_fragment"))
        .context("glimm fragment function")?;
    descriptor.setVertexFunction(Some(&vertex));
    descriptor.setFragmentFunction(Some(&fragment));
    unsafe { descriptor.colorAttachments().objectAtIndexedSubscript(0) }.setPixelFormat(format);
    device
        .newRenderPipelineStateWithDescriptor_error(&descriptor)
        .context("creating glimm pipeline")
}
#[cfg(test)]
mod tests;
