// glimm 0.4.1 shader.ts and GlimmScope FS2, MIT; see docs/GLIMM-LICENSE.
cbuffer GlimmUniforms : register(b0) { float4 v[13]; };
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
float4 glimm_fragment(float4 position : SV_Position) : SV_Target {
    float2 px = (position.xy - v[1].xy) / v[0].z;
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
