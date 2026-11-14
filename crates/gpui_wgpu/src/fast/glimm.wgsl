// glimm 0.4.1 shader.ts and GlimmScope FS2, MIT; see docs/GLIMM-LICENSE.
struct GlimmUniforms { v: array<vec4<f32>, 18>, }
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
    // False-colour views for development (GlimmDebugView); 0 draws the transition.
    let debug = u.v[9].w;
    if debug > 3.5 { return vec4(crest, trough, span_mask, 1.0); }
    if debug > 2.5 { return vec4(vec3(clamp(blur / 32.0, 0.0, 1.0)), 1.0); }
    if debug > 1.5 { return vec4(clamp(vec2(0.5) + (sp - px) / 64.0, vec2(0.0), vec2(1.0)), 0.5, 1.0); }
    if debug > 0.5 { return vec4(vec3(mix_new), 1.0); }
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
// Radial distortions: Emerge (mode 1), Shock (2), Collapse (3). Rows 13 to 17 are described at
// radial_rows in crates/gpui/src/fast/glimm/mod.rs; lengths are logical pixels. Every function
// returns a radius along the ray from the point of action: content at that radius shows here.

// The travelling wave: a raised ring at the front that magnifies what lies under it, ripples
// trailing behind it, and for Shock an echo ring at six tenths of its radius. The ring widens
// with its amplitude and the ripples are capped by their wavelength, so neither folds the
// content over itself.
fn wave_at(r: f32) -> f32 {
    let radius = u.v[13].y;
    let amplitude = u.v[13].w;
    if amplitude <= 0.0 { return 0.0; }
    let width = max(u.v[13].z, 3.3 * amplitude);
    let wavelength = u.v[14].x;
    let train = u.v[14].y;
    let falloff = u.v[14].z;
    let echo = u.v[14].w;
    let x = (r - radius) / width;
    let lens = -2.3316 * x * exp(-x * x);
    let behind = max((radius - r) * select(1.0, -1.0, u.v[13].x > 2.5), 0.0);
    var ripples = 0.0;
    if train > 0.0 {
        ripples = sin(6.28318 * behind / wavelength) * exp(-behind / train) * smoothstep(0.0, 0.5 * width, behind);
    }
    let e = (r - 0.6 * radius) / width;
    let second = echo * -2.3316 * e * exp(-e * e);
    return (amplitude * (lens + second) + min(0.6 * amplitude, 0.1 * wavelength) * ripples) * falloff / (falloff + r);
}
// The hold while work runs: content drawn toward the point, deeper on each in-breath, with a
// ripple leaving the point once a breath.
fn hold_at(r: f32) -> f32 {
    let hold = u.v[16].x;
    if hold <= 0.0 { return 0.0; }
    let q = r / u.v[16].z;
    let phase = u.v[16].y;
    let breath = 0.5 - 0.5 * cos(6.28318 * phase);
    let pull = u.v[16].z * (0.07 + 0.07 * breath) * 2.3316 * q * exp(-q * q);
    let ripple = 3.0 * sin(6.28318 * (1.6 * q - phase)) * exp(-0.35 * q * q) * smoothstep(0.0, 0.4, q);
    return hold * (pull + ripple);
}
// One layer of content. Inside the front it is squeezed toward the point: the content out to
// the scope's edge, `reach` away along this ray, is pulled into the bubble, most strongly at
// its rim. Once the front has passed that edge the content springs past its place and back,
// a damped wobble that dies with the wave. Outside the front the content is pushed aside, or
// pulled in after it when push < 0.
fn layer_at(r: f32, reach: f32, inside: bool) -> f32 {
    let radius = u.v[13].y;
    let width = u.v[13].z;
    var shifted: f32;
    if inside {
        var q = 0.0;
        if radius > 0.5 { q = clamp(r / radius, 0.0, 1.0); }
        // How far the front has gone past this ray's end, as a share of what it has left to go.
        let rest = u.v[17].w + 2.0 * width + u.v[14].y - reach;
        let past = clamp((radius - reach) / max(rest, 1.0), 0.0, 1.0);
        let spring = sin(7.854 * past) * exp(-3.0 * past) * min(u.v[17].x * 2.5, 1.0)
            * 0.4 * width * r / max(reach, 1.0);
        shifted = r + u.v[15].w * (max(reach - radius, 0.0) * q * q - spring);
    } else {
        let push = u.v[16].w;
        let decay = 2.0 * width;
        let d = abs(push) * decay * min(radius / decay, 1.0) * exp(-max(r - radius, 0.0) / decay);
        shifted = select(r + d, r - d, push >= 0.0);
    }
    return shifted + wave_at(r) + hold_at(r);
}
// A layer sampled along the ray over `spread` pixels. The share of the spread that is
// dispersion splits the colours: weights run from red through green to blue, or through the
// palette as tint rises; the rest is an even smear. Weights sum to white, so where nothing
// bends the colour comes back exactly.
fn spectral(at: vec2<f32>, n: vec2<f32>, src: f32, spread: f32, share: f32, lod: f32, hue: f32, is_new: bool) -> vec4<f32> {
    if abs(spread) < 0.35 { return page(at + n * src, is_new, lod); }
    let tint = u.v[15].y;
    var rgb = vec3(0.0);
    var total = vec3(0.0);
    var alpha = 0.0;
    for (var i = 0; i < 5; i++) {
        let t = f32(i) * 0.25;
        let basis = clamp(vec3(1.0 - 2.0 * t, 1.0 - abs(2.0 * t - 1.0), 2.0 * t - 1.0), vec3(0.0), vec3(1.0));
        let w = mix(vec3(1.0), mix(basis, max(palette(0.8 * t + hue), vec3(0.02)), tint), share);
        let c = page(at + n * (src + (t - 0.5) * spread), is_new, lod);
        rgb += c.rgb * w;
        total += w;
        alpha += c.a;
    }
    return vec4(rgb / total, alpha * 0.2);
}
// One layer seen through the warp at a pixel `r` from the point along `n`: its colour, where
// it was sampled, how far its colours spread and its blur level.
struct Layer { color: vec4<f32>, shown: vec2<f32>, spread: f32, lod: f32, }
fn radial_layer(r: f32, n: vec2<f32>, reach: f32, edge: f32, wave: f32, hue: f32, inside: bool, is_new: bool) -> Layer {
    let at = u.v[2].xy;
    let blur = u.v[15].z;
    let src = r + (layer_at(r, reach, inside) - r) * edge;
    let next = r + 1.0 + (layer_at(r + 1.0, reach, inside) - r - 1.0) * edge;
    // Stretch along and across the ray: above 1 the content is squeezed, below it magnified.
    let along = next - src;
    let across = select(1.0, src / r, r > 1.0);
    let squeezed = max(max(along, across), 1.0);
    let stretched = max(squeezed, 1.0 / max(min(along, across), 0.05));
    let dispersion = u.v[15].x * abs(wave);
    let smear = blur * min(stretched - 1.0, 6.0) * 1.5;
    let lod = blur * log2(squeezed);
    let spread = select(-1.0, 1.0, wave >= 0.0) * (dispersion + smear);
    let color = spectral(at, n, src, spread, dispersion / (dispersion + smear + 0.001), lod, hue, is_new);
    return Layer(color, at + n * src, spread, lod);
}
fn radial(px: vec2<f32>) -> vec4<f32> {
    let res = u.v[0].xy;
    let at = u.v[2].xy;
    let mode = u.v[13].x;
    let radius = u.v[13].y;
    let width = u.v[13].z;
    let d = px - at;
    let r = length(d);
    var n = vec2(0.0);
    if r > 0.001 { n = d / r; }
    // Where this ray leaves the scope, so how much content lies along it.
    let safe = select(n, vec2(1e-4), abs(n) < vec2(1e-4));
    let exits = max(-at / safe, (res - at) / safe);
    let reach = max(min(exits.x, exits.y), r);
    // Displacement dies out at the scope's edges, where its content meets what surrounds it,
    // and never reaches past them to repeat the edge's pixels.
    let edges = min(px, res - px);
    let edge = smoothstep(0.0, max(u.v[7].z, 6.0), min(edges.x, edges.y));
    // Emerge shows new content inside the front and Collapse outside it; Shock has no split.
    let split = mode < 1.5 || mode > 2.5;
    let inward = mode > 2.5;
    let x = (r - radius) / width;
    let zone = 0.3 + 1.0 / width;
    var inside = 0.0;
    if split { inside = 1.0 - smoothstep(-zone, zone, x); }
    let wave = wave_at(r) * edge;
    // The hue drifts across the ring and with time; a function of the ray, so it has no seam.
    let hue = u.v[7].y + 0.15 * (n.x - n.y) + u.v[0].w * 0.04;
    var within = Layer(vec4(0.0), px, 0.0, 0.0);
    var beyond = Layer(vec4(0.0), px, 0.0, 0.0);
    if inside > 0.001 { within = radial_layer(r, n, reach, edge, wave, hue, true, !inward); }
    if inside < 0.999 { beyond = radial_layer(r, n, reach, edge, wave, hue, false, inward || !split); }
    var col = within.color * inside + beyond.color * (1.0 - inside);
    // The rim: the front's leading edge catches the light in the palette's colours, faintly on
    // its own and brightly where it refracts bright content, most where it tilts to the light.
    let energy = u.v[17].x;
    let crest = exp(-x * x);
    let slope = -2.0 * x * crest * min(u.v[13].w / width, 1.0) * edge;
    let normal = normalize(vec3(-n * slope * 1.6, 1.0));
    let halfway = normalize(normalize(vec3(-0.35, -0.55, 0.9)) + vec3(0.0, 0.0, 1.0));
    let spec = pow(max(dot(normal, halfway), 0.0), 48.0);
    let lead = select(x, -x, inward) - 0.55;
    let rim = exp(-lead * lead * 25.0);
    let luma = dot(col.rgb, vec3(0.299, 0.587, 0.114));
    let glow = u.v[15].y * energy * edge * rim * (0.12 + 0.9 * luma + 0.5 * spec);
    col = vec4(col.rgb + palette(hue + 0.15 * x) * u.v[6].w * glow * col.a, col.a);
    // The optional iridescent band, drawn as a ring on the front over the refracted content.
    let overlay = clamp(u.v[17].y * exp(-x * x * 2.0) * edge, 0.0, 1.0);
    if overlay > 0.0 {
        let t = hue + 0.2 * x;
        let c = (palette(t) * 0.5 + palette(t - 0.18) * 0.25 + palette(t + 0.18) * 0.25) * u.v[6].w;
        col = vec4(c * overlay, overlay) + col * (1.0 - overlay);
    }
    // False-colour views (GlimmDebugView), as for the glimm.dev content above.
    let debug = u.v[9].w;
    var shown = beyond;
    if inside > 0.5 { shown = within; }
    if debug > 3.5 { return vec4(crest, abs(wave_at(r)) / max(u.v[13].w, 1.0), clamp(hold_at(r) / 16.0, 0.0, 1.0), 1.0); }
    if debug > 2.5 { return vec4(vec3(clamp(max(exp2(shown.lod) - 1.0, abs(shown.spread)) / 32.0, 0.0, 1.0)), 1.0); }
    if debug > 1.5 { return vec4(clamp(vec2(0.5) + (shown.shown - px) / 64.0, vec2(0.0), vec2(1.0)), 0.5, 1.0); }
    if debug > 0.5 {
        var mixed = 1.0;
        if split { mixed = select(inside, 1.0 - inside, inward); }
        return vec4(vec3(mixed), 1.0);
    }
    return vec4(min(col.rgb, vec3(col.a)), col.a);
}
@fragment fn glimm_fragment(in: Vertex) -> @location(0) vec4<f32> {
    let px = (in.position.xy - u.v[1].xy) / u.v[0].z;
    if u.v[13].x > 0.5 {
        let warped = radial(px);
        if warped.a > 0.00001 { return vec4(warped.rgb / warped.a, warped.a); }
        return vec4(0.0, 0.0, 0.0, warped.a);
    }
    var page_color: vec4<f32>;
    if u.v[7].w > 0.5 { page_color = content(px); }
    else { page_color = page(px, true, 0.0); }
    // A debug view replaces the frame, band included, so its colours stay readable.
    if u.v[9].w > 0.5 && u.v[7].w > 0.5 { return page_color; }
    let band_color = band(px);
    let result = band_color + page_color * (1.0 - band_color.a);
    // The sprite pipeline consumes straight alpha, unlike the scoped raster.
    if result.a > 0.00001 { return vec4(result.rgb / result.a, result.a); }
    return vec4(0.0, 0.0, 0.0, result.a);
}
