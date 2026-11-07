// glimm 0.4.1 shader.ts and GlimmScope FS2, MIT; see docs/GLIMM-LICENSE.
struct GlimmUniforms { v: array<vec4<f32>, 13>, }
@group(0) @binding(0) var<uniform> u: GlimmUniforms;
@group(0) @binding(1) var live: texture_2d<f32>;
@group(0) @binding(2) var old: texture_2d<f32>;
@group(0) @binding(3) var glimm_sampler: sampler;

struct Vertex { @builtin(position) position: vec4<f32>, @location(0) uv: vec2<f32>, }
@vertex fn glimm_vertex(@builtin(vertex_index) id: u32) -> Vertex {
    let positions = array<vec2<f32>, 3>(vec2(-1.0, -1.0), vec2(3.0, -1.0), vec2(-1.0, 3.0));
    let p = positions[id];
    return Vertex(vec4(p, 0.0, 1.0), vec2(p.x * 0.5 + 0.5, 0.5 - p.y * 0.5));
}
@fragment fn mipmap_fragment(in: Vertex) -> @location(0) vec4<f32> {
    return textureSampleLevel(live, glimm_sampler, in.uv, 0.0);
}
fn page(p: vec2<f32>, is_new: bool, lod: f32) -> vec4<f32> {
    let local = clamp(p / u.v[0].xy, vec2(0.0), vec2(1.0));
    let uv = (u.v[1].zw + local * u.v[0].xy * u.v[0].z) / vec2<f32>(textureDimensions(live));
    if is_new { return textureSampleLevel(live, glimm_sampler, uv, lod); }
    return textureSampleLevel(old, glimm_sampler, uv, lod);
}
fn soft(p: vec2<f32>, is_new: bool, radius: f32) -> vec4<f32> {
    if radius < 0.4 { return page(p, is_new, 0.0); }
    let lod = max(0.0, log2(radius * u.v[0].z / 5.0));
    var acc = page(p, is_new, lod);
    for (var i = 1u; i < 16u; i++) {
        let fi = f32(i);
        let angle = fi * 2.39996;
        acc += page(p + vec2(cos(angle), sin(angle)) * sqrt(fi / 16.0) * radius, is_new, lod);
    }
    return acc / 16.0;
}
fn content(px: vec2<f32>) -> vec4<f32> {
    let res = u.v[0].xy;
    let origin = u.v[2].xy;
    let dir = u.v[2].zw;
    let gather = u.v[3].z;
    let haze = u.v[3].w;
    let ring = u.v[4].x;
    let scale = u.v[4].y;
    let dim = u.v[4].z;
    let burst = u.v[4].w;
    let curve = u.v[5].y;
    let span = u.v[5].z;
    let center = u.v[5].w;
    let front = u.v[8].z;
    let width = u.v[8].w;
    let d = px - origin;
    let dist = length(d);
    let shorter = min(res.x, res.y);
    let near_origin = exp(-dist / (0.45 * shorter));
    var sp = px + d * gather * 0.16 * near_origin;
    var blur = haze * 16.0 * scale * near_origin * near_origin * smoothstep(48.0, 48.0 + 0.22 * shorter, dist);
    let horizontal = abs(dir.x) > 0.5;
    let length_axis = select(res.y, res.x, horizontal);
    let al = dot(px, dir) + select(0.0, length_axis, dir.x + dir.y < 0.0);
    let ac = select(px.x, px.y, horizontal);
    let across = select(res.x, res.y, horizontal);
    let c = (ac / across - center) / (0.5 * span);
    let cc = clamp(c, -1.0, 1.0);
    let span_mask = select(1.0 - smoothstep(0.45, 1.0, abs(c)), 1.0, span >= 1.0);
    let lag = curve * cc * cc * length_axis;
    let band_u = (al - (front - lag)) / width;
    let crest = exp(-band_u * band_u);
    let trough = exp(-pow((band_u + 2.6) / 1.4, 2.0));
    let push = (crest * 22.0 - trough * 8.0) * ring * span_mask * scale;
    sp -= dir * push;
    let bu = band_u * 0.5;
    blur += 14.0 * scale * ring * span_mask * exp(-pow((bu + 0.2) / 0.9, 2.0));
    let zone = width * (1.2 + 3.0 * (1.0 - span_mask));
    let mix_new = select(1.0 - smoothstep(-zone, zone, al - (front - lag * ring)), 1.0, burst < 0.5);
    var col: vec4<f32>;
    if mix_new > 0.999 { col = soft(sp, true, blur); }
    else if mix_new < 0.001 { col = soft(sp, false, blur); }
    else { col = mix(soft(sp, false, blur), soft(sp, true, blur), mix_new); }
    let split = crest * ring * span_mask * 2.0 * scale;
    if split > 0.3 {
        let lod = max(0.0, log2(blur * u.v[0].z / 5.0));
        col.r = mix(page(sp + dir * split, false, lod).r, page(sp + dir * split, true, lod).r, mix_new);
        col.b = mix(page(sp - dir * split, false, lod).b, page(sp - dir * split, true, lod).b, mix_new);
    }
    let rgb = col.rgb * (1.0 - 0.09 * dim * gather * (1.0 - near_origin));
    return vec4(min(rgb, vec3(col.a)), col.a);
}
fn palette(t: f32) -> vec3<f32> {
    return u.v[9].xyz + u.v[10].xyz * cos(6.28318530718 * (u.v[11].xyz * t + u.v[12].xyz));
}
fn band(px: vec2<f32>) -> vec4<f32> {
    let top_uv = px / u.v[0].xy;
    // GLSL gl_FragCoord is bottom-up; WebGPU fragment positions are top-down.
    let uv = vec2(top_uv.x, 1.0 - top_uv.y);
    let vertical = abs(u.v[2].w) > 0.5;
    let axis = select(uv.x, uv.y, vertical);
    let across = select(uv.y, uv.x, vertical);
    let progress = u.v[3].x;
    let alpha = u.v[3].y;
    let tight = u.v[5].x;
    let curve = u.v[5].y;
    let span = u.v[5].z;
    let center = select(1.0 - u.v[5].w, u.v[5].w, vertical);
    let pos = mix(u.v[8].x, u.v[8].y, progress);
    let time = u.v[0].w;
    let tw = time * u.v[6].z;
    let wave = sin(across * 4.2 + tw * 0.08 + 0.3) * 0.004 * u.v[6].x;
    let c = (across - center) / max(0.5 * span, 0.001);
    let cc = clamp(c, -1.0, 1.0);
    let lag = curve * cc * cc;
    let span_mask = select(1.0 - smoothstep(0.45, 1.0, abs(c)), 1.0, span >= 1.0);
    let d = (axis - pos) - wave + sign(u.v[8].y - u.v[8].x) * lag;
    let intensity_band = exp(-d * d * tight);
    let derivative = -2.0 * d * tight * intensity_band;
    let slope = select(vec2(derivative, 0.0), vec2(0.0, derivative), vertical);
    let n = normalize(vec3(-slope.x * 0.18, slope.y * 0.18, 1.0));
    let behind = max(-d * sign(u.v[8].y - u.v[8].x), 0.0);
    let trail = smoothstep(0.0, 0.08, behind) * (1.0 - smoothstep(0.12, 0.65, behind)) * 0.10;
    let focus = 4.0 * progress * (1.0 - progress);
    let halo = exp(-d * d * 2.5) * 0.05 * focus;
    let intensity = clamp(max(intensity_band, trail) * span_mask + halo * (1.0 - intensity_band), 0.0, 1.0);
    let ripple = sin(across * 12.0 + axis * 3.0 + tw * 0.40) * 0.015 * u.v[6].y;
    let t = n.x * 0.12 + n.y * 0.08 + axis * 0.90 + across * 0.16 + ripple + u.v[7].y + time * 0.04;
    let col = (palette(t) * 0.50 + palette(t - 0.18) * 0.25 + palette(t + 0.18) * 0.25) * u.v[6].w;
    let view = vec3(0.0, 0.0, 1.0);
    let light = normalize(vec3(0.35, 0.55, 0.9));
    let halfway = normalize(light + view);
    let fresnel = pow(1.0 - clamp(dot(n, view), 0.0, 1.0), 3.0);
    let spec = pow(clamp(dot(n, halfway), 0.0, 1.0), 80.0);
    let entry = mix(0.2, 1.0, focus);
    let body_a = intensity * alpha * entry;
    let high_mask = intensity_band * span_mask * alpha * entry * u.v[7].x;
    let emit = (col * fresnel * 0.55 + vec3(spec) * 1.1) * high_mask;
    let high_a = (fresnel * 0.4 + spec * 0.9) * high_mask;
    let edges = min(px, u.v[0].xy - px);
    var feather = 1.0;
    if u.v[7].z > 0.0 { feather = smoothstep(0.0, u.v[7].z, min(edges.x, edges.y)); }
    return vec4(col * body_a + emit, min(body_a + high_a, 1.0)) * feather;
}
@fragment fn glimm_fragment(in: Vertex) -> @location(0) vec4<f32> {
    let px = (in.position.xy - u.v[1].xy) / u.v[0].z;
    var page_color: vec4<f32>;
    if u.v[7].w > 0.5 { page_color = content(px); }
    else { page_color = page(px, true, 0.0); }
    let band_color = band(px);
    let result = band_color + page_color * (1.0 - band_color.a);
    // The sprite pipeline consumes straight alpha, unlike the scoped raster.
    if result.a > 0.00001 { return vec4(result.rgb / result.a, result.a); }
    return vec4(0.0, 0.0, 0.0, result.a);
}
