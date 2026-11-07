use anyhow::{Context as _, Result};
use gpui::Scene;
use windows::Win32::{
    Foundation::HMODULE,
    Graphics::{
        Direct3D::{D3D_DRIVER_TYPE_WARP, D3D_FEATURE_LEVEL_10_1, D3D_FEATURE_LEVEL_11_0},
        Direct3D11::{
            D3D11_CPU_ACCESS_READ, D3D11_CREATE_DEVICE_BGRA_SUPPORT, D3D11_CULL_NONE,
            D3D11_FILL_SOLID, D3D11_MAP_READ, D3D11_MAPPED_SUBRESOURCE, D3D11_RASTERIZER_DESC,
            D3D11_SDK_VERSION, D3D11_TEXTURE2D_DESC, D3D11_USAGE_STAGING, D3D11_VIEWPORT,
            D3D11CreateDevice, ID3D11Device, ID3D11DeviceContext, ID3D11RasterizerState,
            ID3D11ShaderResourceView,
        },
    },
};

use super::{GlimmRenderer, Texture};
use crate::{
    DirectXAtlas,
    directx_renderer::{DirectXGlobalElements, DirectXRenderPipelines},
    fast::{
        frame::{self, FrameState, Target},
        layers::tile_cache::create_globals,
    },
};

struct Renderer {
    device: ID3D11Device,
    context: ID3D11DeviceContext,
    atlas: DirectXAtlas,
    globals: DirectXGlobalElements,
    pipelines: DirectXRenderPipelines,
    rasterizer: ID3D11RasterizerState,
    frame: FrameState,
    glimm: GlimmRenderer,
}

fn renderer() -> Renderer {
    let mut device = None;
    let mut context = None;
    unsafe {
        D3D11CreateDevice(
            None,
            D3D_DRIVER_TYPE_WARP,
            HMODULE::default(),
            D3D11_CREATE_DEVICE_BGRA_SUPPORT,
            Some(&[D3D_FEATURE_LEVEL_11_0, D3D_FEATURE_LEVEL_10_1]),
            D3D11_SDK_VERSION,
            Some(&mut device),
            None,
            Some(&mut context),
        )
        .expect("Glimm tests require D3D11 WARP");
    }
    let device = device.unwrap();
    let context = context.unwrap();
    let mut rasterizer = None;
    unsafe {
        device
            .CreateRasterizerState(
                &D3D11_RASTERIZER_DESC {
                    FillMode: D3D11_FILL_SOLID,
                    CullMode: D3D11_CULL_NONE,
                    DepthClipEnable: true.into(),
                    MultisampleEnable: true.into(),
                    ..Default::default()
                },
                Some(&mut rasterizer),
            )
            .unwrap();
    }
    let atlas = DirectXAtlas::new(&device, &context);
    let pipelines = DirectXRenderPipelines::new(&device).unwrap();
    let globals = DirectXGlobalElements::new(&device).unwrap();
    Renderer {
        device,
        context,
        atlas,
        globals,
        pipelines,
        rasterizer: rasterizer.unwrap(),
        frame: FrameState::default(),
        glimm: GlimmRenderer::default(),
    }
}
fn cache_counts(renderer: &Renderer) -> (bool, usize, usize) {
    let state = &renderer.glimm;
    (state.live.is_some(), state.old.len(), state.outputs.len())
}
fn backend_name() -> &'static str {
    "directx-warp"
}

fn render(renderer: &mut Renderer, scene: &Scene) -> image::RgbaImage {
    let mut state = std::mem::take(&mut renderer.glimm);
    let device = renderer.device.clone();
    let context = renderer.context.clone();
    let composed = if scene.fast_glimm.frames.is_empty() {
        state.clear();
        None
    } else {
        Some(
            super::compose(
                &mut state,
                scene,
                &device,
                &context,
                [512, 320],
                |content, texture, images| draw_into(renderer, content, texture, images, [0.; 4]),
            )
            .unwrap(),
        )
    };
    let texture = Texture::new(&device, [512, 320], false).unwrap();
    draw_into(
        renderer,
        composed.as_ref().unwrap_or(scene),
        &texture,
        &state.images,
        [1.; 4],
    )
    .unwrap();
    let pixels = read_back(&device, &context, &texture).unwrap();
    renderer.glimm = state;
    image::RgbaImage::from_raw(512, 320, pixels).unwrap()
}

fn draw_into(
    renderer: &mut Renderer,
    scene: &Scene,
    texture: &Texture,
    images: &[ID3D11ShaderResourceView],
    clear: [f32; 4],
) -> Result<()> {
    let globals = DirectXGlobalElements {
        global_params_buffer: create_globals(&renderer.device, texture.size[0], texture.size[1])?,
        batch_params_buffer: renderer.globals.batch_params_buffer.clone(),
        sampler: renderer.globals.sampler.clone(),
    };
    let target = Target {
        glimm_images: images,
        device: &renderer.device,
        device_context: &renderer.context,
        atlas: &renderer.atlas,
        globals: &globals,
        paths: None,
    };
    frame::upload(&target, &mut renderer.pipelines, scene)?;
    let context = &renderer.context;
    unsafe {
        context.PSSetShaderResources(0, Some(&[None, None]));
        context.OMSetRenderTargets(Some(&[Some(texture.view.clone())]), None);
        context.ClearRenderTargetView(&texture.view, &clear);
        context.RSSetState(&renderer.rasterizer);
        context.RSSetViewports(Some(&[D3D11_VIEWPORT {
            Width: texture.size[0] as f32,
            Height: texture.size[1] as f32,
            MaxDepth: 1.,
            ..Default::default()
        }]));
        context.VSSetConstantBuffers(0, Some(std::slice::from_ref(&globals.global_params_buffer)));
        context.PSSetConstantBuffers(0, Some(std::slice::from_ref(&globals.global_params_buffer)));
        context.VSSetConstantBuffers(1, Some(std::slice::from_ref(&globals.batch_params_buffer)));
    }
    frame::draw_scene(&target, &mut renderer.pipelines, &mut renderer.frame, scene)?;
    unsafe {
        context.OMSetRenderTargets(None, None);
        context.PSSetShaderResources(0, Some(&[None, None]));
    }
    Ok(())
}

fn read_back(
    device: &ID3D11Device,
    context: &ID3D11DeviceContext,
    texture: &Texture,
) -> Result<Vec<u8>> {
    let mut desc = D3D11_TEXTURE2D_DESC::default();
    unsafe { texture.texture.GetDesc(&mut desc) };
    let mut staging = None;
    unsafe {
        device.CreateTexture2D(
            &D3D11_TEXTURE2D_DESC {
                Usage: D3D11_USAGE_STAGING,
                BindFlags: 0,
                CPUAccessFlags: D3D11_CPU_ACCESS_READ.0 as u32,
                MiscFlags: 0,
                ..desc
            },
            None,
            Some(&mut staging),
        )?;
    }
    let staging = staging.context("Glimm readback texture")?;
    let mut pixels = vec![0u8; desc.Width as usize * desc.Height as usize * 4];
    unsafe {
        context.CopyResource(&staging, &texture.texture);
        let mut mapped = D3D11_MAPPED_SUBRESOURCE::default();
        context.Map(&staging, 0, D3D11_MAP_READ, 0, Some(&mut mapped))?;
        // The staging mapping holds RowPitch * height bytes until Unmap.
        for row in 0..desc.Height as usize {
            std::ptr::copy_nonoverlapping(
                (mapped.pData as *const u8).add(row * mapped.RowPitch as usize),
                pixels.as_mut_ptr().add(row * desc.Width as usize * 4),
                desc.Width as usize * 4,
            );
        }
        context.Unmap(&staging, 0);
    }
    for pixel in pixels.chunks_exact_mut(4) {
        pixel.swap(0, 2);
    }
    Ok(pixels)
}

#[path = "../../../../gpui/src/fast/glimm/gpu_tests.rs"]
mod shared;

#[test]
fn glimm_gpu_device_recovery_discards_the_old_devices_textures() {
    let mut first = renderer();
    let scene = shared::parity_scene();
    let expected = render(&mut first, &scene);
    let mut second = renderer();
    second.glimm = std::mem::take(&mut first.glimm);
    let actual = render(&mut second, &scene);
    assert_eq!(expected, actual);
    assert_eq!(second.glimm.device.as_ref(), Some(&second.device));
    assert_eq!(cache_counts(&second), (true, 1, 1));
}
