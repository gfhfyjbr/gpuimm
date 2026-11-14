// Port of glimm 0.4.1's shader.ts and GlimmScope FS2. See docs/GLIMM-LICENSE.
#include <metal_stdlib>
using namespace metal;

struct GlimmUniforms { float4 v[18]; };
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
// Radial distortions: Emerge (mode 1), Shock (2), Collapse (3). Rows 13 to 17 are described at
// radial_rows in crates/gpui/src/fast/glimm/mod.rs; lengths are logical pixels. Every function
// returns a radius along the ray from the point of action: content at that radius shows here.

// The travelling wave: a raised ring at the front that magnifies what lies under it, ripples
// trailing behind it, and for Shock an echo ring at six tenths of its radius. The ring widens
// with its amplitude and the ripples are capped by their wavelength, so neither folds the
// content over itself.
float glimm_wave(float r, constant GlimmUniforms &u) {
    float radius = u.v[13].y, amplitude = u.v[13].w;
    if (amplitude <= 0.0) return 0.0;
    float width = max(u.v[13].z,3.3*amplitude);
    float wavelength = u.v[14].x, train = u.v[14].y, falloff = u.v[14].z, echo = u.v[14].w;
    float x = (r-radius)/width;
    float lens = -2.3316*x*exp(-x*x);
    float behind = max((radius-r)*(u.v[13].x>2.5 ? -1.0 : 1.0),0.0f);
    float ripples = train>0.0 ? sin(6.28318*behind/wavelength)*exp(-behind/train)*smoothstep(0.0f,0.5f*width,behind) : 0.0;
    float e = (r-0.6*radius)/width;
    float second = echo*-2.3316*e*exp(-e*e);
    return (amplitude*(lens+second)+min(0.6*amplitude,0.1*wavelength)*ripples)*falloff/(falloff+r);
}
// The hold while work runs: content drawn toward the point, deeper on each in-breath, with a
// ripple leaving the point once a breath.
float glimm_hold(float r, constant GlimmUniforms &u) {
    float hold = u.v[16].x;
    if (hold <= 0.0) return 0.0;
    float q = r/u.v[16].z, phase = u.v[16].y;
    float breath = 0.5-0.5*cos(6.28318*phase);
    float pull = u.v[16].z*(0.07+0.07*breath)*2.3316*q*exp(-q*q);
    float ripple = 3.0*sin(6.28318*(1.6*q-phase))*exp(-0.35*q*q)*smoothstep(0.0f,0.4f,q);
    return hold*(pull+ripple);
}
// One layer of content. Inside the front it is squeezed toward the point: the content out to
// the scope's edge, `reach` away along this ray, is pulled into the bubble, most strongly at
// its rim. Once the front has passed that edge the content springs past its place and back,
// a damped wobble that dies with the wave. Outside the front the content is pushed aside, or
// pulled in after it when push < 0.
float glimm_layer(float r, float reach, bool inside, constant GlimmUniforms &u) {
    float radius = u.v[13].y, width = u.v[13].z;
    float shifted;
    if (inside) {
        float q = radius>0.5 ? clamp(r/radius,0.0f,1.0f) : 0.0;
        // How far the front has gone past this ray's end, as a share of what it has left to go.
        float rest = u.v[17].w+2.0*width+u.v[14].y-reach;
        float past = clamp((radius-reach)/max(rest,1.0f),0.0f,1.0f);
        float spring = sin(7.854*past)*exp(-3.0*past)*min(u.v[17].x*2.5,1.0f)
                       *0.4*width*r/max(reach,1.0f);
        shifted = r+u.v[15].w*(max(reach-radius,0.0f)*q*q-spring);
    } else {
        float push = u.v[16].w, decay = 2.0*width;
        float d = abs(push)*decay*min(radius/decay,1.0f)*exp(-max(r-radius,0.0f)/decay);
        shifted = push>=0.0 ? r-d : r+d;
    }
    return shifted+glimm_wave(r,u)+glimm_hold(r,u);
}
// A layer sampled along the ray over `spread` pixels. The share of the spread that is
// dispersion splits the colours: weights run from red through green to blue, or through the
// palette as tint rises; the rest is an even smear. Weights sum to white, so where nothing
// bends the colour comes back exactly.
float4 glimm_spectral(float2 at, float2 n, float src, float spread, float share, float lod,
                      float hue, bool is_new, constant GlimmUniforms &u,
                      texture2d<float> live, texture2d<float> old) {
    if (abs(spread)<0.35) return glimm_page(at+n*src,is_new,lod,u,live,old);
    float tint = u.v[15].y;
    float3 rgb = float3(0), total = float3(0);
    float alpha = 0.0;
    for (int i=0; i<5; i++) {
        float t = float(i)*0.25;
        float3 basis = clamp(float3(1.0-2.0*t,1.0-abs(2.0*t-1.0),2.0*t-1.0),0.0f,1.0f);
        float3 w = mix(float3(1),mix(basis,max(glimm_palette(0.8*t+hue,u),float3(0.02)),tint),share);
        float4 c = glimm_page(at+n*(src+(t-0.5)*spread),is_new,lod,u,live,old);
        rgb += c.rgb*w;
        total += w;
        alpha += c.a;
    }
    return float4(rgb/total,alpha*0.2);
}
// One layer seen through the warp at a pixel `r` from the point along `n`: its colour, where
// it was sampled, how far its colours spread and its blur level.
struct GlimmLayer { float4 color; float2 shown; float spread; float lod; };
GlimmLayer glimm_radial_layer(float r, float2 n, float reach, float edge, float wave, float hue,
                              bool inside, bool is_new, constant GlimmUniforms &u,
                              texture2d<float> live, texture2d<float> old) {
    float2 at = u.v[2].xy;
    float blur = u.v[15].z;
    float src = r+(glimm_layer(r,reach,inside,u)-r)*edge;
    float next = r+1.0+(glimm_layer(r+1.0,reach,inside,u)-r-1.0)*edge;
    // Stretch along and across the ray: above 1 the content is squeezed, below it magnified.
    float radial = next-src, tangential = r>1.0 ? src/r : 1.0;
    float squeezed = max(max(radial,tangential),1.0);
    float stretched = max(squeezed,1.0/max(min(radial,tangential),0.05f));
    float dispersion = u.v[15].x*abs(wave), smear = blur*min(stretched-1.0,6.0f)*1.5;
    GlimmLayer layer;
    layer.lod = blur*log2(squeezed);
    layer.spread = (wave>=0.0 ? 1.0 : -1.0)*(dispersion+smear);
    layer.shown = at+n*src;
    layer.color = glimm_spectral(at,n,src,layer.spread,dispersion/(dispersion+smear+0.001),
                                 layer.lod,hue,is_new,u,live,old);
    return layer;
}
float4 glimm_radial(float2 px, constant GlimmUniforms &u,
                    texture2d<float> live, texture2d<float> old) {
    float2 res = u.v[0].xy, at = u.v[2].xy;
    float mode = u.v[13].x, radius = u.v[13].y, width = u.v[13].z;
    float2 d = px-at;
    float r = length(d);
    float2 n = r>0.001 ? d/r : float2(0);
    // Where this ray leaves the scope, so how much content lies along it.
    float2 safe = float2(abs(n.x)<1e-4 ? 1e-4 : n.x, abs(n.y)<1e-4 ? 1e-4 : n.y);
    float2 exits = max(-at/safe,(res-at)/safe);
    float reach = max(min(exits.x,exits.y),r);
    // Displacement dies out at the scope's edges, where its content meets what surrounds it,
    // and never reaches past them to repeat the edge's pixels.
    float2 edges = min(px,res-px);
    float edge = smoothstep(0.0f,max(u.v[7].z,6.0f),min(edges.x,edges.y));
    // Emerge shows new content inside the front and Collapse outside it; Shock has no split.
    bool split = mode<1.5 || mode>2.5;
    bool inward = mode>2.5;
    float x = (r-radius)/width;
    float zone = 0.3+1.0/width;
    float inside = split ? 1.0-smoothstep(-zone,zone,x) : 0.0;
    float wave = glimm_wave(r,u)*edge;
    // The hue drifts across the ring and with time; a function of the ray, so it has no seam.
    float hue = u.v[7].y+0.15*(n.x-n.y)+u.v[0].w*0.04;
    GlimmLayer within = {float4(0),px,0.0,0.0}, beyond = {float4(0),px,0.0,0.0};
    if (inside>0.001) within = glimm_radial_layer(r,n,reach,edge,wave,hue,true,!inward,u,live,old);
    if (inside<0.999) beyond = glimm_radial_layer(r,n,reach,edge,wave,hue,false,inward || !split,u,live,old);
    float4 col = within.color*inside+beyond.color*(1.0-inside);
    // The rim: the front's leading edge catches the light in the palette's colours, faintly on
    // its own and brightly where it refracts bright content, most where it tilts to the light.
    float energy = u.v[17].x;
    float crest = exp(-x*x);
    float slope = -2.0*x*crest*min(u.v[13].w/width,1.0f)*edge;
    float3 normal = normalize(float3(-n*slope*1.6,1.0));
    float3 halfway = normalize(normalize(float3(-0.35,-0.55,0.9))+float3(0,0,1));
    float spec = pow(max(dot(normal,halfway),0.0f),48.0);
    float lead = (inward ? -x : x)-0.55;
    float rim = exp(-lead*lead*25.0);
    float luma = dot(col.rgb,float3(0.299,0.587,0.114));
    float glow = u.v[15].y*energy*edge*rim*(0.12+0.9*luma+0.5*spec);
    col.rgb += glimm_palette(hue+0.15*x,u)*u.v[6].w*glow*col.a;
    // The optional iridescent band, drawn as a ring on the front over the refracted content.
    float band = clamp(u.v[17].y*exp(-x*x*2.0)*edge,0.0f,1.0f);
    if (band>0.0) {
        float t = hue+0.2*x;
        float3 c = (glimm_palette(t,u)*0.5+glimm_palette(t-0.18,u)*0.25+glimm_palette(t+0.18,u)*0.25)*u.v[6].w;
        col = float4(c*band,band)+col*(1.0-band);
    }
    // False-colour views (GlimmDebugView), as for the glimm.dev content above.
    float debug = u.v[9].w;
    GlimmLayer main = inside>0.5 ? within : beyond;
    if (debug>3.5) return float4(crest,abs(glimm_wave(r,u))/max(u.v[13].w,1.0f),clamp(glimm_hold(r,u)/16.0,0.0f,1.0f),1);
    if (debug>2.5) return float4(float3(clamp(max(exp2(main.lod)-1.0,abs(main.spread))/32.0,0.0f,1.0f)),1);
    if (debug>1.5) return float4(clamp(float2(0.5)+(main.shown-px)/64.0,float2(0),float2(1)),0.5,1);
    if (debug>0.5) return float4(float3(split ? (inward ? 1.0-inside : inside) : 1.0),1);
    return float4(min(col.rgb,float3(col.a)),col.a);
}
fragment float4 glimm_fragment(GlimmVertex in [[stage_in]], constant GlimmUniforms &u [[buffer(0)]],
    texture2d<float> live [[texture(0)]], texture2d<float> old [[texture(1)]]) {
    float2 px = (in.position.xy-u.v[1].xy)/u.v[0].z;
    if (u.v[13].x>0.5) {
        float4 radial = glimm_radial(px,u,live,old);
        return float4(radial.a>0.00001 ? radial.rgb/radial.a : float3(0),radial.a);
    }
    float4 content = u.v[7].w>0.5 ? glimm_content(px,u,live,old) : glimm_page(px,true,0,u,live,old);
    // A debug view replaces the frame, band included, so its colours stay readable.
    if (u.v[9].w>0.5 && u.v[7].w>0.5) return content;
    float4 band = glimm_band(px,u);
    float4 result = band+content*(1.0-band.a);
    // GPUI's polychrome pipeline expects straight alpha; the reference shader is premultiplied.
    return float4(result.a>0.00001 ? result.rgb/result.a : float3(0),result.a);
}
