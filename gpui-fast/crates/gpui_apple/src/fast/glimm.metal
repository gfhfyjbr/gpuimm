// Port of glimm 0.4.1's shader.ts and GlimmScope FS2. See docs/GLIMM-LICENSE.
#include <metal_stdlib>
using namespace metal;

struct GlimmUniforms { float4 v[13]; };
struct GlimmVertex { float4 position [[position]]; };

vertex GlimmVertex glimm_vertex(uint id [[vertex_id]]) {
    const float2 positions[3] = { float2(-1,-1), float2(3,-1), float2(-1,3) };
    return {float4(positions[id],0,1)};
}

constexpr sampler glimm_sampler(coord::normalized, address::clamp_to_edge,
    min_filter::linear, mag_filter::linear, mip_filter::linear);

float4 glimm_page(float2 p, bool is_new, float lod, constant GlimmUniforms &u,
                 texture2d<float> live, texture2d<float> old) {
    float2 local = clamp(p/u.v[0].xy,float2(0),float2(1));
    float2 uv = (u.v[1].zw+local*u.v[0].xy*u.v[0].z)/float2(live.get_width(),live.get_height());
    return is_new ? live.sample(glimm_sampler,uv,level(lod)) : old.sample(glimm_sampler,uv,level(lod));
}
float4 glimm_soft(float2 p, bool is_new, float radius, constant GlimmUniforms &u,
                 texture2d<float> live, texture2d<float> old) {
    if (radius < 0.4) return glimm_page(p,is_new,0,u,live,old);
    float lod = max(0.0f,log2(radius*u.v[0].z/5.0));
    float4 acc = glimm_page(p,is_new,lod,u,live,old);
    for (int i=1; i<16; i++) {
        float fi = float(i), angle = fi*2.39996;
        acc += glimm_page(p+float2(cos(angle),sin(angle))*sqrt(fi/16.0)*radius,is_new,lod,u,live,old);
    }
    return acc/16.0;
}
float4 glimm_content(float2 px, constant GlimmUniforms &u,
                     texture2d<float> live, texture2d<float> old) {
    float2 res = u.v[0].xy, origin = u.v[2].xy, dir = u.v[2].zw;
    float gather = u.v[3].z, haze = u.v[3].w;
    float ring = u.v[4].x, scale = u.v[4].y, dim = u.v[4].z, burst = u.v[4].w;
    float curve = u.v[5].y, span = u.v[5].z, center = u.v[5].w;
    float front = u.v[8].z, width = u.v[8].w;
    float2 d = px-origin;
    float dist = length(d), shorter = min(res.x,res.y);
    float near_origin = exp(-dist/(0.45*shorter));
    float2 sp = px+d*gather*0.16*near_origin;
    float blur = haze*16.0*scale*near_origin*near_origin*smoothstep(48.0f,48.0+0.22*shorter,dist);
    bool horizontal = abs(dir.x)>0.5;
    float length_axis = horizontal ? res.x : res.y;
    float al = dot(px,dir)+(dir.x+dir.y<0.0 ? length_axis : 0.0);
    float ac = horizontal ? px.y : px.x;
    float across = horizontal ? res.y : res.x;
    float c = (ac/across-center)/(0.5*span);
    float cc = clamp(c,-1.0f,1.0f);
    float span_mask = span>=1.0 ? 1.0 : 1.0-smoothstep(0.45f,1.0f,abs(c));
    float lag = curve*cc*cc*length_axis;
    float band_u = (al-(front-lag))/width;
    float crest = exp(-band_u*band_u);
    float trough = exp(-pow((band_u+2.6)/1.4,2.0));
    float push = (crest*22.0-trough*8.0)*ring*span_mask*scale;
    sp -= dir*push;
    float bu = band_u*0.5;
    blur += 14.0*scale*ring*span_mask*exp(-pow((bu+0.2)/0.9,2.0));
    float zone = width*(1.2+3.0*(1.0-span_mask));
    float mix_new = burst<0.5 ? 1.0 : 1.0-smoothstep(-zone,zone,al-(front-lag*ring));
    float4 col;
    if (mix_new>0.999) col = glimm_soft(sp,true,blur,u,live,old);
    else if (mix_new<0.001) col = glimm_soft(sp,false,blur,u,live,old);
    else col = mix(glimm_soft(sp,false,blur,u,live,old),glimm_soft(sp,true,blur,u,live,old),mix_new);
    float split = crest*ring*span_mask*2.0*scale;
    if (split>0.3) {
        float lod = max(0.0f,log2(blur*u.v[0].z/5.0));
        col.r = mix(glimm_page(sp+dir*split,false,lod,u,live,old).r,glimm_page(sp+dir*split,true,lod,u,live,old).r,mix_new);
        col.b = mix(glimm_page(sp-dir*split,false,lod,u,live,old).b,glimm_page(sp-dir*split,true,lod,u,live,old).b,mix_new);
    }
    col.rgb *= 1.0-0.09*dim*gather*(1.0-near_origin);
    // False-colour views for development (GlimmDebugView); 0 draws the transition.
    float debug = u.v[9].w;
    if (debug > 3.5) return float4(crest,trough,span_mask,1);
    if (debug > 2.5) return float4(float3(clamp(blur/32.0,0.0f,1.0f)),1);
    if (debug > 1.5) return float4(clamp(float2(0.5)+(sp-px)/64.0,float2(0),float2(1)),0.5,1);
    if (debug > 0.5) return float4(float3(mix_new),1);
    return float4(min(col.rgb,float3(col.a)),col.a);
}
float3 glimm_palette(float t, constant GlimmUniforms &u) {
    return u.v[9].xyz+u.v[10].xyz*cos(6.28318530718*(u.v[11].xyz*t+u.v[12].xyz));
}
float4 glimm_band(float2 px, constant GlimmUniforms &u) {
    float2 uv = px/u.v[0].xy;
    uv.y = 1.0-uv.y; // GLSL's gl_FragCoord is bottom-up; Metal's is top-down.
    bool vertical = abs(u.v[2].w)>0.5;
    float axis = vertical ? uv.y : uv.x, cross = vertical ? uv.x : uv.y;
    float progress = u.v[3].x, alpha = u.v[3].y;
    float tight = u.v[5].x, curve = u.v[5].y, span = u.v[5].z;
    float center = vertical ? u.v[5].w : 1.0-u.v[5].w;
    float pos = mix(u.v[8].x,u.v[8].y,progress);
    float time = u.v[0].w, tw = time*u.v[6].z;
    float wave = sin(cross*4.2+tw*0.08+0.3)*0.004*u.v[6].x;
    float c = (cross-center)/max(0.5*span,0.001f);
    float cc = clamp(c,-1.0f,1.0f);
    float lag = curve*cc*cc;
    float span_mask = span>=1.0 ? 1.0 : 1.0-smoothstep(0.45f,1.0f,abs(c));
    float d = (axis-pos)-wave+sign(u.v[8].y-u.v[8].x)*lag;
    float band = exp(-d*d*tight);
    float derivative = -2.0*d*tight*band;
    float2 slope = vertical ? float2(0,derivative) : float2(derivative,0);
    float3 n = normalize(float3(-slope.x*0.18,slope.y*0.18,1.0));
    float behind = max(-d*sign(u.v[8].y-u.v[8].x),0.0f);
    float trail = smoothstep(0.0f,0.08f,behind)*(1.0-smoothstep(0.12f,0.65f,behind))*0.10;
    float focus = 4.0*progress*(1.0-progress);
    float halo = exp(-d*d*2.5)*0.05*focus;
    float intensity = clamp(max(band,trail)*span_mask+halo*(1.0-band),0.0f,1.0f);
    float ripple = sin(cross*12.0+axis*3.0+tw*0.40)*0.015*u.v[6].y;
    float t = n.x*0.12+n.y*0.08+axis*0.90+cross*0.16+ripple+u.v[7].y+time*0.04;
    float3 col = (glimm_palette(t,u)*0.50+glimm_palette(t-0.18,u)*0.25+glimm_palette(t+0.18,u)*0.25)*u.v[6].w;
    float3 view = float3(0,0,1), light = normalize(float3(0.35,0.55,0.9));
    float3 halfway = normalize(light+view);
    float fresnel = pow(1.0-clamp(dot(n,view),0.0f,1.0f),3.0);
    float spec = pow(clamp(dot(n,halfway),0.0f,1.0f),80.0);
    float entry = mix(0.2f,1.0f,focus);
    float body_a = intensity*alpha*entry;
    float high_mask = band*span_mask*alpha*entry*u.v[7].x;
    float3 emit = (col*fresnel*0.55+float3(spec)*1.1)*high_mask;
    float high_a = (fresnel*0.4+spec*0.9)*high_mask;
    float2 edges = min(px,u.v[0].xy-px);
    float feather = u.v[7].z>0.0 ? smoothstep(0.0f,u.v[7].z,min(edges.x,edges.y)) : 1.0;
    return float4(col*body_a+emit,min(body_a+high_a,1.0f))*feather;
}
fragment float4 glimm_fragment(GlimmVertex in [[stage_in]], constant GlimmUniforms &u [[buffer(0)]],
    texture2d<float> live [[texture(0)]], texture2d<float> old [[texture(1)]]) {
    float2 px = (in.position.xy-u.v[1].xy)/u.v[0].z;
    float4 content = u.v[7].w>0.5 ? glimm_content(px,u,live,old) : glimm_page(px,true,0,u,live,old);
    // A debug view replaces the frame, band included, so its colours stay readable.
    if (u.v[9].w>0.5 && u.v[7].w>0.5) return content;
    float4 band = glimm_band(px,u);
    float4 result = band+content*(1.0-band.a);
    // GPUI's polychrome pipeline expects straight alpha; the reference shader is premultiplied.
    return float4(result.a>0.00001 ? result.rgb/result.a : float3(0),result.a);
}
