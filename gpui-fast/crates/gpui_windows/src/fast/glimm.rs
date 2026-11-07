use std::{ffi::CStr, ops::Range, slice};

use anyhow::{Context as _, Result};
use gpui::{
    AtlasTextureId, AtlasTextureKind, AtlasTile, Bounds, ContentMask, Corners, DevicePixels,
    GlimmFrame, GlimmUniforms, PolychromeSprite, ScaledPixels, Scene, TileId,
    WindowBackgroundAppearance, point, size,
};
use windows::{
    Win32::{
        Foundation::RECT,
        Graphics::{
            Direct3D::{D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST, Fxc::D3DCompile, ID3DBlob},
            Direct3D11::{
                D3D11_BIND_CONSTANT_BUFFER, D3D11_BIND_RENDER_TARGET, D3D11_BIND_SHADER_RESOURCE,
                D3D11_BOX, D3D11_BUFFER_DESC, D3D11_COMPARISON_NEVER, D3D11_CULL_NONE,
                D3D11_FILL_SOLID, D3D11_FILTER_MIN_MAG_MIP_LINEAR, D3D11_RASTERIZER_DESC,
                D3D11_RESOURCE_MISC_GENERATE_MIPS, D3D11_SAMPLER_DESC, D3D11_TEXTURE_ADDRESS_CLAMP,
                D3D11_TEXTURE2D_DESC, D3D11_USAGE_DEFAULT, D3D11_VIEWPORT, ID3D11Buffer,
                ID3D11Device, ID3D11DeviceContext, ID3D11PixelShader, ID3D11RasterizerState,
                ID3D11RenderTargetView, ID3D11SamplerState, ID3D11ShaderResourceView,
                ID3D11Texture2D, ID3D11VertexShader,
            },
            Dxgi::Common::DXGI_SAMPLE_DESC,
        },
    },
    core::PCSTR,
};

use crate::directx_renderer::{DirectXRenderer, RENDER_TARGET_FORMAT};

const TEXTURE_BASE: u32 = 0xd000_0000;

#[derive(Default)]
pub(crate) struct GlimmRenderer {
    rendering: bool,
    device: Option<ID3D11Device>,
    pipeline: Option<Pipeline>,
    live: Option<Texture>,
    old: Vec<(u64, Texture)>,
    current: Vec<(u64, Texture)>,
    outputs: Vec<(u64, Texture)>,
    pub(crate) images: Vec<ID3D11ShaderResourceView>,
    viewport: [u32; 2],
}

struct Texture {
    texture: ID3D11Texture2D,
    view: ID3D11RenderTargetView,
    srv: ID3D11ShaderResourceView,
    size: [u32; 2],
}

impl Texture {
    fn new(device: &ID3D11Device, size: [u32; 2], mipped: bool) -> Result<Self> {
        let desc = D3D11_TEXTURE2D_DESC {
            Width: size[0],
            Height: size[1],
            MipLevels: if mipped {
                size[0].max(size[1]).ilog2() + 1
            } else {
                1
            },
            ArraySize: 1,
            Format: RENDER_TARGET_FORMAT,
            SampleDesc: DXGI_SAMPLE_DESC {
                Count: 1,
                Quality: 0,
            },
            Usage: D3D11_USAGE_DEFAULT,
            BindFlags: (D3D11_BIND_RENDER_TARGET.0 | D3D11_BIND_SHADER_RESOURCE.0) as u32,
            MiscFlags: if mipped {
                D3D11_RESOURCE_MISC_GENERATE_MIPS.0 as u32
            } else {
                0
            },
            ..Default::default()
        };
        let mut texture = None;
        let mut view = None;
        let mut srv = None;
        unsafe {
            device.CreateTexture2D(&desc, None, Some(&mut texture))?;
            let texture = texture.as_ref().context("Glimm texture")?;
            device.CreateRenderTargetView(texture, None, Some(&mut view))?;
            device.CreateShaderResourceView(texture, None, Some(&mut srv))?;
        }
        Ok(Self {
            texture: texture.context("Glimm texture")?,
            view: view.context("Glimm render target")?,
            srv: srv.context("Glimm shader resource")?,
            size,
        })
    }
}

struct Pipeline {
    vertex: ID3D11VertexShader,
    fragment: ID3D11PixelShader,
    sampler: ID3D11SamplerState,
    rasterizer: ID3D11RasterizerState,
    uniforms: ID3D11Buffer,
}

impl Pipeline {
    fn new(device: &ID3D11Device) -> Result<Self> {
        let vs = compile(c"glimm_vertex", c"vs_4_1")?;
        let ps = compile(c"glimm_fragment", c"ps_4_1")?;
        let mut vertex = None;
        let mut fragment = None;
        let mut sampler = None;
        let mut rasterizer = None;
        let mut uniforms = None;
        unsafe {
            // ID3DBlob owns both byte slices throughout shader creation; D3D11 copies them.
            device.CreateVertexShader(
                slice::from_raw_parts(vs.GetBufferPointer().cast(), vs.GetBufferSize()),
                None,
                Some(&mut vertex),
            )?;
            device.CreatePixelShader(
                slice::from_raw_parts(ps.GetBufferPointer().cast(), ps.GetBufferSize()),
                None,
                Some(&mut fragment),
            )?;
            device.CreateSamplerState(
                &D3D11_SAMPLER_DESC {
                    Filter: D3D11_FILTER_MIN_MAG_MIP_LINEAR,
                    AddressU: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressV: D3D11_TEXTURE_ADDRESS_CLAMP,
                    AddressW: D3D11_TEXTURE_ADDRESS_CLAMP,
                    MaxAnisotropy: 1,
                    ComparisonFunc: D3D11_COMPARISON_NEVER,
                    MaxLOD: f32::MAX,
                    ..Default::default()
                },
                Some(&mut sampler),
            )?;
            device.CreateRasterizerState(
                &D3D11_RASTERIZER_DESC {
                    FillMode: D3D11_FILL_SOLID,
                    CullMode: D3D11_CULL_NONE,
                    DepthClipEnable: true.into(),
                    ScissorEnable: true.into(),
                    ..Default::default()
                },
                Some(&mut rasterizer),
            )?;
            device.CreateBuffer(
                &D3D11_BUFFER_DESC {
                    ByteWidth: size_of::<GlimmUniforms>() as u32,
                    Usage: D3D11_USAGE_DEFAULT,
                    BindFlags: D3D11_BIND_CONSTANT_BUFFER.0 as u32,
                    ..Default::default()
                },
                None,
                Some(&mut uniforms),
            )?;
        }
        Ok(Self {
            vertex: vertex.context("Glimm vertex shader")?,
            fragment: fragment.context("Glimm fragment shader")?,
            sampler: sampler.context("Glimm sampler")?,
            rasterizer: rasterizer.context("Glimm rasterizer")?,
            uniforms: uniforms.context("Glimm uniforms")?,
        })
    }
}

fn compile(entry: &CStr, target: &CStr) -> Result<ID3DBlob> {
    let source = include_str!("glimm.hlsl");
    let mut code = None;
    let mut errors = None;
    let result = unsafe {
        D3DCompile(
            source.as_ptr().cast(),
            source.len(),
            PCSTR::null(),
            None,
            None,
            PCSTR(entry.as_ptr().cast()),
            PCSTR(target.as_ptr().cast()),
            0,
            0,
            &mut code,
            Some(&mut errors),
        )
    };
    result.map_err(|error| {
        let diagnostic = errors
            .map(|blob| unsafe {
                // The compiler's diagnostic blob is valid while `blob` is retained here.
                String::from_utf8_lossy(slice::from_raw_parts(
                    blob.GetBufferPointer().cast(),
                    blob.GetBufferSize(),
                ))
                .trim_end_matches('\0')
                .to_owned()
            })
            .unwrap_or_default();
        anyhow::anyhow!(
            "Glimm HLSL {}: {error}: {diagnostic}",
            entry.to_string_lossy()
        )
    })?;
    code.context("Glimm compiler returned no shader")
}

pub(crate) fn is_texture(id: AtlasTextureId) -> bool {
    id.kind == AtlasTextureKind::Polychrome && id.index & 0xf000_0000 == TEXTURE_BASE
}
pub(crate) fn image(
    images: &[ID3D11ShaderResourceView],
    id: AtlasTextureId,
) -> Option<&ID3D11ShaderResourceView> {
    if !is_texture(id) {
        return None;
    }
    images.get((id.index & 0x0fff_ffff) as usize)
}

impl GlimmRenderer {
    fn clear(&mut self) {
        self.live = None;
        self.old.clear();
        self.current.clear();
        self.outputs.clear();
        self.images.clear();
    }
}

pub(crate) fn render(
    renderer: &mut DirectXRenderer,
    scene: &Scene,
    background: WindowBackgroundAppearance,
) -> Result<bool> {
    if renderer.fast_glimm.rendering {
        return Ok(false);
    }
    if scene.fast_glimm.frames.is_empty() {
        renderer.fast_glimm.clear();
        return Ok(false);
    }
    let Some(devices) = renderer.devices.as_ref() else {
        renderer.fast_glimm = GlimmRenderer::default();
        return Ok(false);
    };
    if renderer
        .resources
        .as_ref()
        .is_none_or(|r| r.render_target_view.is_none())
    {
        return Ok(false);
    }
    let device = devices.device.clone();
    let context = devices.device_context.clone();
    let viewport = [renderer.width, renderer.height];
    let mut state = std::mem::take(&mut renderer.fast_glimm);
    renderer.fast_glimm.rendering = true;
    let result = compose(
        &mut state,
        scene,
        &device,
        &context,
        viewport,
        |content, raster, images| {
            renderer.fast_glimm.images = images.to_vec();
            let previous = std::mem::replace(
                &mut renderer.resources.as_mut().unwrap().render_target_view,
                Some(raster.view.clone()),
            );
            let result = renderer.render(content, WindowBackgroundAppearance::Transparent);
            renderer.resources.as_mut().unwrap().render_target_view = previous;
            result
        },
    );
    state.rendering = true;
    renderer.fast_glimm = state;
    let result = result.and_then(|composed| renderer.render(&composed, background));
    renderer.fast_glimm.rendering = false;
    result.map(|()| true)
}

fn compose(
    state: &mut GlimmRenderer,
    scene: &Scene,
    device: &ID3D11Device,
    context: &ID3D11DeviceContext,
    viewport: [u32; 2],
    mut rasterize: impl FnMut(&Scene, &Texture, &[ID3D11ShaderResourceView]) -> Result<()>,
) -> Result<Scene> {
    anyhow::ensure!(viewport[0] > 0 && viewport[1] > 0, "invalid Glimm viewport");
    if state.device.as_ref() != Some(device) {
        *state = GlimmRenderer::default();
        state.device = Some(device.clone());
    }
    if state.viewport != viewport {
        state.clear();
        state.viewport = viewport;
    }
    if state.pipeline.is_none() {
        state.pipeline = Some(Pipeline::new(device)?);
    }
    if state.live.is_none() {
        state.live = Some(Texture::new(device, viewport, false)?);
    }
    state.images.clear();
    state
        .old
        .retain(|(id, _)| scene.fast_glimm.frames.iter().any(|f| f.run_id == *id));
    state
        .current
        .retain(|(id, _)| scene.fast_glimm.frames.iter().any(|f| f.run_id == *id));
    state
        .outputs
        .retain(|(id, _)| scene.fast_glimm.frames.iter().any(|f| f.run_id == *id));
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
        let crop = crop(frame, viewport);
        let dimensions = [crop[2], crop[3]];
        if state
            .current
            .iter()
            .any(|(id, texture)| *id == frame.run_id && texture.size != dimensions)
        {
            state.current.retain(|(id, _)| *id != frame.run_id);
            state.old.retain(|(id, _)| *id != frame.run_id);
        }
        let raster = state.live.as_ref().unwrap();
        if !state.old.iter().any(|(id, _)| *id == frame.run_id) {
            let texture = Texture::new(device, dimensions, true)?;
            rasterize(&frame.old_scene, raster, &state.images)?;
            copy_crop(context, raster, &texture, crop);
            state.old.push((frame.run_id, texture));
        }
        if !state.current.iter().any(|(id, _)| *id == frame.run_id) {
            state
                .current
                .push((frame.run_id, Texture::new(device, dimensions, true)?));
        }
        if !state.outputs.iter().any(|(id, _)| *id == frame.run_id) {
            state
                .outputs
                .push((frame.run_id, Texture::new(device, viewport, false)?));
        }
        let live = &state
            .current
            .iter()
            .find(|(id, _)| *id == frame.run_id)
            .unwrap()
            .1;
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
        rasterize(&content, raster, &state.images)?;
        copy_crop(context, raster, live, crop);
        paint(
            context,
            state.pipeline.as_ref().unwrap(),
            frame,
            live,
            old,
            output,
            crop,
        )?;
        let index = state.images.len();
        state.images.push(output.srv.clone());
        let mut next = Scene::default();
        next.replay(0..range.start, &composed);
        next.insert_primitive(sprite(index, frame.clip, viewport));
        next.replay(range.end..composed.len(), &composed);
        next.fast_glimm.frames.clear();
        composed = next;
        for pending in &mut frames[i + 1..] {
            rebase(&mut pending.paint_range, &range)?;
        }
    }
    composed.finish();
    Ok(composed)
}

fn crop(frame: &GlimmFrame, viewport: [u32; 2]) -> [u32; 4] {
    let x = (frame.bounds.origin.x.0.floor().max(0.) as u32).min(viewport[0] - 1);
    let y = (frame.bounds.origin.y.0.floor().max(0.) as u32).min(viewport[1] - 1);
    let right = frame.bounds.right().0.ceil().max(0.) as u32;
    let bottom = frame.bounds.bottom().0.ceil().max(0.) as u32;
    [
        x,
        y,
        right.min(viewport[0]).saturating_sub(x).max(1),
        bottom.min(viewport[1]).saturating_sub(y).max(1),
    ]
}
fn copy_crop(context: &ID3D11DeviceContext, source: &Texture, target: &Texture, crop: [u32; 4]) {
    let [x, y, width, height] = crop;
    unsafe {
        context.OMSetRenderTargets(None, None);
        context.PSSetShaderResources(0, Some(&[None, None]));
        context.CopySubresourceRegion(
            &target.texture,
            0,
            0,
            0,
            0,
            &source.texture,
            0,
            Some(&D3D11_BOX {
                left: x,
                top: y,
                front: 0,
                right: x + width,
                bottom: y + height,
                back: 1,
            }),
        );
        context.GenerateMips(&target.srv);
    }
}
fn paint(
    context: &ID3D11DeviceContext,
    pipeline: &Pipeline,
    frame: &GlimmFrame,
    live: &Texture,
    old: &Texture,
    output: &Texture,
    crop: [u32; 4],
) -> Result<()> {
    let mut uniforms = frame.uniforms;
    uniforms.0[1][2] = frame.bounds.origin.x.0 - crop[0] as f32;
    uniforms.0[1][3] = frame.bounds.origin.y.0 - crop[1] as f32;
    let clip = frame.clip;
    let left = clip.origin.x.0.floor().max(0.) as u32;
    let top = clip.origin.y.0.floor().max(0.) as u32;
    let right = (clip.right().0.ceil().max(0.) as u32).min(output.size[0]);
    let bottom = (clip.bottom().0.ceil().max(0.) as u32).min(output.size[1]);
    unsafe {
        let previous_rasterizer = context.RSGetState().ok();
        context.PSSetShaderResources(0, Some(&[None, None]));
        context.OMSetRenderTargets(Some(&[Some(output.view.clone())]), None);
        context.ClearRenderTargetView(&output.view, &[0.; 4]);
        if right > left && bottom > top {
            // UpdateSubresource copies all thirteen initialized float4 rows immediately.
            context.UpdateSubresource(
                &pipeline.uniforms,
                0,
                None,
                uniforms.0.as_ptr().cast(),
                0,
                0,
            );
            context.IASetInputLayout(None);
            context.IASetPrimitiveTopology(D3D_PRIMITIVE_TOPOLOGY_TRIANGLELIST);
            context.VSSetShader(&pipeline.vertex, None);
            context.PSSetShader(&pipeline.fragment, None);
            context.PSSetConstantBuffers(0, Some(&[Some(pipeline.uniforms.clone())]));
            context.PSSetSamplers(0, Some(&[Some(pipeline.sampler.clone())]));
            context.PSSetShaderResources(0, Some(&[Some(live.srv.clone()), Some(old.srv.clone())]));
            context.OMSetBlendState(None, None, u32::MAX);
            context.RSSetState(&pipeline.rasterizer);
            context.RSSetViewports(Some(&[D3D11_VIEWPORT {
                Width: output.size[0] as f32,
                Height: output.size[1] as f32,
                MaxDepth: 1.,
                ..Default::default()
            }]));
            context.RSSetScissorRects(Some(&[RECT {
                left: left as i32,
                top: top as i32,
                right: right as i32,
                bottom: bottom as i32,
            }]));
            context.Draw(3, 0);
        }
        context.PSSetShaderResources(0, Some(&[None, None]));
        context.OMSetRenderTargets(None, None);
        context.RSSetState(previous_rasterizer.as_ref());
    }
    Ok(())
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

#[cfg(test)]
mod tests;
