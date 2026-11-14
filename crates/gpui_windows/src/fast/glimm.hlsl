// glimm 0.4.1 shader.ts and GlimmScope FS2, MIT; see docs/GLIMM-LICENSE.
cbuffer GlimmUniforms : register(b0) { float4 v[18]; };
Texture2D<float4> live_page : register(t0);
Texture2D<float4> old_page : register(t1);
SamplerState glimm_sampler : register(s0);

float4 glimm_vertex(uint id : SV_VertexID) : SV_Position {
    float2 positions[3] = {float2(-1,-1), float2(3,-1), float2(-1,3)};
    return float4(positions[id], 0, 1);
}
float4 page(float2 p, bool is_new, float lod) {
    uint width, height;
    live_page.GetDimensions(width, height);
    float2 uv = (v[1].zw + clamp(p / v[0].xy, 0.0, 1.0) * v[0].xy * v[0].z) / float2(width, height);
    if (is_new) return live_page.SampleLevel(glimm_sampler, uv, lod);
    return old_page.SampleLevel(glimm_sampler, uv, lod);
}
float4 soft(float2 p, bool is_new, float radius) {
    if (radius < .4) return page(p, is_new, 0);
    float lod = max(0.0, log2(radius * v[0].z / 5.0));
    float4 acc = page(p, is_new, lod);
    [unroll] for (int i = 1; i < 16; ++i) {
        float a = float(i) * 2.39996;
        acc += page(p + float2(cos(a), sin(a)) * sqrt(float(i) / 16.0) * radius, is_new, lod);
    }
    return acc / 16.0;
}
float4 content(float2 px) {
    float2 res = v[0].xy, origin = v[2].xy, dir = v[2].zw;
    float gather = v[3].z, haze = v[3].w, ring = v[4].x;
    float scale = v[4].y, dim = v[4].z, burst = v[4].w;
    float curve = v[5].y, span = v[5].z, center = v[5].w;
    float front = v[8].z, width = v[8].w;
    float2 d = px - origin;
    float dist = length(d), shorter = min(res.x, res.y);
    float near_origin = exp(-dist / (.45 * shorter));
    float2 sp = px + d * gather * .16 * near_origin;
    float blur = haze * 16.0 * scale * near_origin * near_origin * smoothstep(48.0, 48.0 + .22 * shorter, dist);
    bool horizontal = abs(dir.x) > .5;
    float len = horizontal ? res.x : res.y;
    float al = dot(px, dir) + ((dir.x + dir.y < 0) ? len : 0);
    float ac = horizontal ? px.y : px.x, across = horizontal ? res.y : res.x;
    float c = (ac / across - center) / (.5 * span), cc = clamp(c, -1.0, 1.0);
    float sm = span >= 1.0 ? 1.0 : 1.0 - smoothstep(.45, 1.0, abs(c));
    float lag = curve * cc * cc * len;
    float u = (al - (front - lag)) / width;
    float crest = exp(-u * u), trough = exp(-pow((u + 2.6) / 1.4, 2.0));
    float push = (crest * 22.0 - trough * 8.0) * ring * sm * scale;
    sp -= dir * push;
    float bu = u * .5;
    blur += 14.0 * scale * ring * sm * exp(-pow((bu + .2) / .9, 2.0));
    float zone = width * (1.2 + 3.0 * (1.0 - sm));
    float mix_new = burst < .5 ? 1.0 : 1.0 - smoothstep(-zone, zone, al - (front - lag * ring));
    float4 col;
    if (mix_new > .999) col = soft(sp, true, blur);
    else if (mix_new < .001) col = soft(sp, false, blur);
    else col = lerp(soft(sp, false, blur), soft(sp, true, blur), mix_new);
    float split = crest * ring * sm * 2.0 * scale;
    if (split > .3) {
        float lod = max(0.0, log2(blur * v[0].z / 5.0));
        col.r = lerp(page(sp + dir * split, false, lod).r, page(sp + dir * split, true, lod).r, mix_new);
        col.b = lerp(page(sp - dir * split, false, lod).b, page(sp - dir * split, true, lod).b, mix_new);
    }
    col.rgb *= 1.0 - .09 * dim * gather * (1.0 - near_origin);
    // False-colour views for development (GlimmDebugView); 0 draws the transition.
    float debug = v[9].w;
    if (debug > 3.5) return float4(crest, trough, sm, 1.0);
    if (debug > 2.5) return float4(saturate(blur / 32.0).xxx, 1.0);
    if (debug > 1.5) return float4(saturate(float2(.5, .5) + (sp - px) / 64.0), .5, 1.0);
    if (debug > 0.5) return float4(mix_new.xxx, 1.0);
    col.rgb = min(col.rgb, col.a);
    return col;
}
float3 palette(float t) {
    return v[9].xyz + v[10].xyz * cos(6.28318530718 * (v[11].xyz * t + v[12].xyz));
}
float4 band(float2 px) {
    float2 top_uv = px / v[0].xy;
    // GLSL gl_FragCoord is bottom-up; Direct3D pixel positions are top-down.
    float2 uv = float2(top_uv.x, 1.0 - top_uv.y);
    bool vertical = abs(v[2].w) > .5;
    float axis = vertical ? uv.y : uv.x, across = vertical ? uv.x : uv.y;
    float progress = v[3].x, alpha = v[3].y, tight = v[5].x, curve = v[5].y, span = v[5].z;
    float center = vertical ? v[5].w : 1.0 - v[5].w;
    float pos = lerp(v[8].x, v[8].y, progress), time = v[0].w, tw = time * v[6].z;
    float wave = sin(across * 4.2 + tw * .08 + .3) * .004 * v[6].x;
    float c = (across - center) / max(.5 * span, .001), cc = clamp(c, -1.0, 1.0);
    float lag = curve * cc * cc;
    float sm = span >= 1.0 ? 1.0 : 1.0 - smoothstep(.45, 1.0, abs(c));
    float d = (axis - pos) - wave + sign(v[8].y - v[8].x) * lag;
    float ib = exp(-d * d * tight), deriv = -2.0 * d * tight * ib;
    float2 slope = vertical ? float2(0, deriv) : float2(deriv, 0);
    float3 n = normalize(float3(-slope.x * .18, slope.y * .18, 1));
    float behind = max(-d * sign(v[8].y - v[8].x), 0.0);
    float trail = smoothstep(0.0, .08, behind) * (1.0 - smoothstep(.12, .65, behind)) * .10;
    float focus = 4.0 * progress * (1.0 - progress), halo = exp(-d * d * 2.5) * .05 * focus;
    float intensity = clamp(max(ib, trail) * sm + halo * (1.0 - ib), 0.0, 1.0);
    float ripple = sin(across * 12.0 + axis * 3.0 + tw * .40) * .015 * v[6].y;
    float t = n.x * .12 + n.y * .08 + axis * .90 + across * .16 + ripple + v[7].y + time * .04;
    float3 col = (palette(t) * .50 + palette(t - .18) * .25 + palette(t + .18) * .25) * v[6].w;
    float3 view = float3(0, 0, 1), light = normalize(float3(.35, .55, .9));
    float3 h = normalize(light + view);
    float fresnel = pow(1.0 - clamp(dot(n, view), 0.0, 1.0), 3.0);
    float spec = pow(clamp(dot(n, h), 0.0, 1.0), 80.0);
    float entry = lerp(.2, 1.0, focus), body_a = intensity * alpha * entry;
    float hi_mask = ib * sm * alpha * entry * v[7].x;
    float3 emit = (col * fresnel * .55 + spec * 1.1) * hi_mask;
    float hi_a = (fresnel * .4 + spec * .9) * hi_mask;
    float2 edges = min(px, v[0].xy - px);
    float feather = v[7].z > 0 ? smoothstep(0.0, v[7].z, min(edges.x, edges.y)) : 1.0;
    return float4(col * body_a + emit, min(body_a + hi_a, 1.0)) * feather;
}
// Radial distortions: Emerge (mode 1), Shock (2), Collapse (3). Rows 13 to 17 are described at
// radial_rows in crates/gpui/src/fast/glimm/mod.rs; lengths are logical pixels. Every function
// returns a radius along the ray from the point of action: content at that radius shows here.

// The travelling wave: a raised ring at the front that magnifies what lies under it, ripples
// trailing behind it, and for Shock an echo ring at six tenths of its radius. The ring widens
// with its amplitude and the ripples are capped by their wavelength, so neither folds the
// content over itself.
float wave_at(float r) {
    float radius = v[13].y, amplitude = v[13].w;
    if (amplitude <= 0.0) return 0.0;
    float width = max(v[13].z, 3.3 * amplitude);
    float wavelength = v[14].x, train = v[14].y, falloff = v[14].z, echo = v[14].w;
    float x = (r - radius) / width;
    float lens = -2.3316 * x * exp(-x * x);
    float behind = max((radius - r) * (v[13].x > 2.5 ? -1.0 : 1.0), 0.0);
    float ripples = train > 0.0 ? sin(6.28318 * behind / wavelength) * exp(-behind / train) * smoothstep(0.0, 0.5 * width, behind) : 0.0;
    float e = (r - 0.6 * radius) / width;
    float second = echo * -2.3316 * e * exp(-e * e);
    return (amplitude * (lens + second) + min(0.6 * amplitude, 0.1 * wavelength) * ripples) * falloff / (falloff + r);
}
// The hold while work runs: content drawn toward the point, deeper on each in-breath, with a
// ripple leaving the point once a breath.
float hold_at(float r) {
    float hold = v[16].x;
    if (hold <= 0.0) return 0.0;
    float q = r / v[16].z, phase = v[16].y;
    float breath = 0.5 - 0.5 * cos(6.28318 * phase);
    float pull = v[16].z * (0.07 + 0.07 * breath) * 2.3316 * q * exp(-q * q);
    float ripple = 3.0 * sin(6.28318 * (1.6 * q - phase)) * exp(-0.35 * q * q) * smoothstep(0.0, 0.4, q);
    return hold * (pull + ripple);
}
// One layer of content. Inside the front it is squeezed toward the point: the content out to
// the scope's edge, `reach` away along this ray, is pulled into the bubble, most strongly at
// its rim. Once the front has passed that edge the content springs past its place and back,
// a damped wobble that dies with the wave. Outside the front the content is pushed aside, or
// pulled in after it when push < 0.
float layer_at(float r, float reach, bool inside) {
    float radius = v[13].y, width = v[13].z;
    float shifted;
    if (inside) {
        float q = radius > 0.5 ? clamp(r / radius, 0.0, 1.0) : 0.0;
        // How far the front has gone past this ray's end, as a share of what it has left to go.
        float rest = v[17].w + 2.0 * width + v[14].y - reach;
        float past = clamp((radius - reach) / max(rest, 1.0), 0.0, 1.0);
        float spring = sin(7.854 * past) * exp(-3.0 * past) * min(v[17].x * 2.5, 1.0)
            * 0.4 * width * r / max(reach, 1.0);
        shifted = r + v[15].w * (max(reach - radius, 0.0) * q * q - spring);
    } else {
        float push = v[16].w, decay = 2.0 * width;
        float d = abs(push) * decay * min(radius / decay, 1.0) * exp(-max(r - radius, 0.0) / decay);
        shifted = push >= 0.0 ? r - d : r + d;
    }
    return shifted + wave_at(r) + hold_at(r);
}
// A layer sampled along the ray over `spread` pixels. The share of the spread that is
// dispersion splits the colours: weights run from red through green to blue, or through the
// palette as tint rises; the rest is an even smear. Weights sum to white, so where nothing
// bends the colour comes back exactly.
float4 spectral(float2 at, float2 n, float src, float spread, float share, float lod, float hue, bool is_new) {
    if (abs(spread) < 0.35) return page(at + n * src, is_new, lod);
    float tint = v[15].y;
    float3 rgb = 0, total = 0;
    float alpha = 0.0;
    [unroll] for (int i = 0; i < 5; ++i) {
        float t = float(i) * 0.25;
        float3 basis = saturate(float3(1.0 - 2.0 * t, 1.0 - abs(2.0 * t - 1.0), 2.0 * t - 1.0));
        float3 w = lerp(float3(1, 1, 1), lerp(basis, max(palette(0.8 * t + hue), 0.02), tint), share);
        float4 c = page(at + n * (src + (t - 0.5) * spread), is_new, lod);
        rgb += c.rgb * w;
        total += w;
        alpha += c.a;
    }
    return float4(rgb / total, alpha * 0.2);
}
// One layer seen through the warp at a pixel `r` from the point along `n`: its colour, where
// it was sampled, how far its colours spread and its blur level.
struct Layer { float4 color; float2 shown; float spread; float lod; };
Layer radial_layer(float r, float2 n, float reach, float edge, float wave, float hue, bool inside, bool is_new) {
    float2 at = v[2].xy;
    float blur = v[15].z;
    float src = r + (layer_at(r, reach, inside) - r) * edge;
    float next = r + 1.0 + (layer_at(r + 1.0, reach, inside) - r - 1.0) * edge;
    // Stretch along and across the ray: above 1 the content is squeezed, below it magnified.
    float along = next - src, across = r > 1.0 ? src / r : 1.0;
    float squeezed = max(max(along, across), 1.0);
    float stretched = max(squeezed, 1.0 / max(min(along, across), 0.05));
    float dispersion = v[15].x * abs(wave), smear = blur * min(stretched - 1.0, 6.0) * 1.5;
    Layer layer;
    layer.lod = blur * log2(squeezed);
    layer.spread = (wave >= 0.0 ? 1.0 : -1.0) * (dispersion + smear);
    layer.shown = at + n * src;
    layer.color = spectral(at, n, src, layer.spread, dispersion / (dispersion + smear + 0.001), layer.lod, hue, is_new);
    return layer;
}
float4 radial(float2 px) {
    float2 res = v[0].xy, at = v[2].xy;
    float mode = v[13].x, radius = v[13].y, width = v[13].z;
    float2 d = px - at;
    float r = length(d);
    float2 n = r > 0.001 ? d / r : float2(0, 0);
    // Where this ray leaves the scope, so how much content lies along it.
    float2 safe = float2(abs(n.x) < 1e-4 ? 1e-4 : n.x, abs(n.y) < 1e-4 ? 1e-4 : n.y);
    float2 exits = max(-at / safe, (res - at) / safe);
    float reach = max(min(exits.x, exits.y), r);
    // Displacement dies out at the scope's edges, where its content meets what surrounds it,
    // and never reaches past them to repeat the edge's pixels.
    float2 edges = min(px, res - px);
    float edge = smoothstep(0.0, max(v[7].z, 6.0), min(edges.x, edges.y));
    // Emerge shows new content inside the front and Collapse outside it; Shock has no split.
    bool split = mode < 1.5 || mode > 2.5;
    bool inward = mode > 2.5;
    float x = (r - radius) / width;
    float zone = 0.3 + 1.0 / width;
    float inside = split ? 1.0 - smoothstep(-zone, zone, x) : 0.0;
    float wave = wave_at(r) * edge;
    // The hue drifts across the ring and with time; a function of the ray, so it has no seam.
    float hue = v[7].y + 0.15 * (n.x - n.y) + v[0].w * 0.04;
    Layer within = (Layer)0, beyond = (Layer)0;
    within.shown = px;
    beyond.shown = px;
    if (inside > 0.001) within = radial_layer(r, n, reach, edge, wave, hue, true, !inward);
    if (inside < 0.999) beyond = radial_layer(r, n, reach, edge, wave, hue, false, inward || !split);
    float4 col = within.color * inside + beyond.color * (1.0 - inside);
    // The rim: the front's leading edge catches the light in the palette's colours, faintly on
    // its own and brightly where it refracts bright content, most where it tilts to the light.
    float energy = v[17].x;
    float crest = exp(-x * x);
    float slope = -2.0 * x * crest * min(v[13].w / width, 1.0) * edge;
    float3 normal = normalize(float3(-n * slope * 1.6, 1.0));
    float3 halfway = normalize(normalize(float3(-0.35, -0.55, 0.9)) + float3(0, 0, 1));
    float spec = pow(max(dot(normal, halfway), 0.0), 48.0);
    float lead = (inward ? -x : x) - 0.55;
    float rim = exp(-lead * lead * 25.0);
    float luma = dot(col.rgb, float3(0.299, 0.587, 0.114));
    float glow = v[15].y * energy * edge * rim * (0.12 + 0.9 * luma + 0.5 * spec);
    col.rgb += palette(hue + 0.15 * x) * v[6].w * glow * col.a;
    // The optional iridescent band, drawn as a ring on the front over the refracted content.
    float overlay = saturate(v[17].y * exp(-x * x * 2.0) * edge);
    if (overlay > 0.0) {
        float t = hue + 0.2 * x;
        float3 c = (palette(t) * 0.5 + palette(t - 0.18) * 0.25 + palette(t + 0.18) * 0.25) * v[6].w;
        col = float4(c * overlay, overlay) + col * (1.0 - overlay);
    }
    // False-colour views (GlimmDebugView), as for the glimm.dev content above.
    float debug = v[9].w;
    Layer shown = beyond;
    if (inside > 0.5) shown = within;
    if (debug > 3.5) return float4(crest, abs(wave_at(r)) / max(v[13].w, 1.0), saturate(hold_at(r) / 16.0), 1.0);
    if (debug > 2.5) return float4(saturate(max(exp2(shown.lod) - 1.0, abs(shown.spread)) / 32.0).xxx, 1.0);
    if (debug > 1.5) return float4(saturate(float2(.5, .5) + (shown.shown - px) / 64.0), .5, 1.0);
    if (debug > 0.5) return float4((split ? (inward ? 1.0 - inside : inside) : 1.0).xxx, 1.0);
    col.rgb = min(col.rgb, col.a);
    return col;
}
float4 glimm_fragment(float4 position : SV_Position) : SV_Target {
    float2 px = (position.xy - v[1].xy) / v[0].z;
    if (v[13].x > .5) {
        float4 warped = radial(px);
        if (warped.a > .00001) warped.rgb /= warped.a;
        else warped.rgb = 0;
        return warped;
    }
    float4 p = v[7].w > .5 ? content(px) : page(px, true, 0);
    // A debug view replaces the frame, band included, so its colours stay readable.
    if (v[9].w > .5 && v[7].w > .5) return p;
    float4 b = band(px);
    float4 result = b + p * (1.0 - b.a);
    // The sprite pipeline consumes straight alpha, unlike the scoped raster.
    if (result.a > .00001) result.rgb /= result.a;
    else result.rgb = 0;
    return result;
}
