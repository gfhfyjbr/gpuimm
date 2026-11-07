'use client';
import { createContext, useContext, useRef, useTransition, useState, useEffect, useMemo, useSyncExternalStore, forwardRef, useCallback, useImperativeHandle } from 'react';
import { jsxs, jsx } from 'react/jsx-runtime';
import { flushSync } from 'react-dom';

// src/core/oklch.ts
var srgbToLinear = (c) => c <= 0.04045 ? c / 12.92 : Math.pow((c + 0.055) / 1.055, 2.4);
var linearToSrgb = (c) => c <= 31308e-7 ? c * 12.92 : 1.055 * Math.pow(c, 1 / 2.4) - 0.055;
var linearRgbToOklab = (r, g, b) => {
  const l = 0.4122214708 * r + 0.5363325363 * g + 0.0514459929 * b;
  const m = 0.2119034982 * r + 0.6806995451 * g + 0.1073969566 * b;
  const s = 0.0883024619 * r + 0.2817188376 * g + 0.6299787005 * b;
  const l_ = Math.cbrt(l);
  const m_ = Math.cbrt(m);
  const s_ = Math.cbrt(s);
  return [
    0.2104542553 * l_ + 0.793617785 * m_ - 0.0040720468 * s_,
    1.9779984951 * l_ - 2.428592205 * m_ + 0.4505937099 * s_,
    0.0259040371 * l_ + 0.7827717662 * m_ - 0.808675766 * s_
  ];
};
var oklabToLinearRgb = (L, a, b) => {
  const l_ = L + 0.3963377774 * a + 0.2158037573 * b;
  const m_ = L - 0.1055613458 * a - 0.0638541728 * b;
  const s_ = L - 0.0894841775 * a - 1.291485548 * b;
  const l = l_ * l_ * l_;
  const m = m_ * m_ * m_;
  const s = s_ * s_ * s_;
  return [
    4.0767416621 * l - 3.3077115913 * m + 0.2309699292 * s,
    -1.2684380046 * l + 2.6097574011 * m - 0.3413193965 * s,
    -0.0041960863 * l - 0.7034186147 * m + 1.707614701 * s
  ];
};
var rgbToOklch = (rgb) => {
  const [L, a, b] = linearRgbToOklab(
    srgbToLinear(rgb[0]),
    srgbToLinear(rgb[1]),
    srgbToLinear(rgb[2])
  );
  return { L, C: Math.sqrt(a * a + b * b), H: Math.atan2(b, a) };
};
var oklchToRgb = ({ L, C, H }) => {
  const a = C * Math.cos(H);
  const b = C * Math.sin(H);
  const [lr, lg, lb] = oklabToLinearRgb(L, a, b);
  const clamp01 = (x) => Math.max(0, Math.min(1, x));
  return [
    clamp01(linearToSrgb(lr)),
    clamp01(linearToSrgb(lg)),
    clamp01(linearToSrgb(lb))
  ];
};
var hexToRgb = (hex) => {
  const n = parseInt(hex.replace("#", ""), 16);
  return [(n >> 16 & 255) / 255, (n >> 8 & 255) / 255, (n & 255) / 255];
};
var lerpHueShortest = (h1, h2, t) => {
  let diff = h2 - h1;
  if (diff > Math.PI) diff -= 2 * Math.PI;
  if (diff < -Math.PI) diff += 2 * Math.PI;
  return h1 + diff * t;
};
var lerpOklch = (a, b, t) => {
  const EPS = 1e-4;
  const ha = a.C < EPS ? b.H : a.H;
  const hb = b.C < EPS ? a.H : b.H;
  return {
    L: a.L + (b.L - a.L) * t,
    C: a.C + (b.C - a.C) * t,
    H: lerpHueShortest(ha, hb, t)
  };
};
function sampleChainRgb(anchors, samples) {
  if (anchors.length === 0) return [];
  if (anchors.length === 1) {
    const rgb = oklchToRgb(anchors[0]);
    return Array.from({ length: samples }, () => rgb);
  }
  const out = [];
  const K = anchors.length;
  for (let i = 0; i < samples; i++) {
    const t = i / (samples - 1);
    const seg = t * (K - 1);
    const segI = Math.min(K - 2, Math.floor(seg));
    const segT = seg - segI;
    out.push(oklchToRgb(lerpOklch(anchors[segI], anchors[segI + 1], segT)));
  }
  return out;
}

// src/core/palettes.ts
var ACCENTS = {
  red: "#FF3D7F",
  // Cherry      — hot coral-pink
  orange: "#FF7A1A",
  // Tangerine   — juicier orange
  yellow: "#FFD600",
  // Sunflower   — pure marigold
  green: "#C2FF3D",
  // Lime        — electric yellow-green
  mint: "#00FFA8",
  // Spearmint   — near-neon mint
  teal: "#00E5D6",
  // Lagoon      — pool turquoise
  cyan: "#1FC8FF",
  // Sky         — clean cerulean
  blue: "#2E70FF",
  // Cobalt      — electric blue
  indigo: "#7B4FFF",
  // Iris        — vibrant violet
  purple: "#D33CFF",
  // Orchid      — bright magenta
  pink: "#FF3DC0",
  // Hibiscus    — hot pink-magenta
  brown: "#D8A87B"
  // Latte       — warm peach neutral
};
Object.fromEntries(
  Object.entries(ACCENTS).map(([k, hex]) => [
    k,
    rgbToOklch(hexToRgb(hex))
  ])
);
function solve3x3(M, v) {
  const det = (m) => m[0][0] * (m[1][1] * m[2][2] - m[1][2] * m[2][1]) - m[0][1] * (m[1][0] * m[2][2] - m[1][2] * m[2][0]) + m[0][2] * (m[1][0] * m[2][1] - m[1][1] * m[2][0]);
  const D = det(M);
  if (Math.abs(D) < 1e-9) return null;
  const col = (i) => M.map((row, r) => row.map((x, c) => c === i ? v[r] : x));
  return [det(col(0)) / D, det(col(1)) / D, det(col(2)) / D];
}
function fitCosinePaletteToSamples(samples) {
  const N = samples.length;
  const C = 0.5;
  const ts = samples.map((_, i) => i / (N - 1));
  const cosV = ts.map((t) => Math.cos(2 * Math.PI * C * t));
  const sinV = ts.map((t) => Math.sin(2 * Math.PI * C * t));
  const sCos = cosV.reduce((s, v) => s + v, 0);
  const sSin = sinV.reduce((s, v) => s + v, 0);
  const sCosCos = cosV.reduce((s, v) => s + v * v, 0);
  const sSinSin = sinV.reduce((s, v) => s + v * v, 0);
  const sCosSin = cosV.reduce((s, v, i) => s + v * sinV[i], 0);
  const M = [
    [N, sCos, sSin],
    [sCos, sCosCos, sCosSin],
    [sSin, sCosSin, sSinSin]
  ];
  const fitChannel = (ch) => {
    const ys = samples.map((c) => c[ch]);
    const sY = ys.reduce((s, y) => s + y, 0);
    const sYCos = ys.reduce((s, y, i) => s + y * cosV[i], 0);
    const sYSin = ys.reduce((s, y, i) => s + y * sinV[i], 0);
    const sol = solve3x3(M, [sY, sYCos, sYSin]);
    if (!sol) return { a: sY / N, b: 0, d: 0 };
    const [a, alpha, beta] = sol;
    const b = Math.sqrt(alpha * alpha + beta * beta);
    const d = (Math.atan2(-beta, alpha) / (2 * Math.PI) + 1) % 1;
    return { a, b, d };
  };
  const r = fitChannel(0);
  const g = fitChannel(1);
  const bl = fitChannel(2);
  return {
    a: [r.a, g.a, bl.a],
    b: [r.b, g.b, bl.b],
    c: [C, C, C],
    d: [r.d, g.d, bl.d]
  };
}
var accentPair = (hexA, hexB) => {
  const samples = sampleChainRgb(
    [rgbToOklch(hexToRgb(hexA)), rgbToOklch(hexToRgb(hexB))],
    17
  );
  samples[0] = hexToRgb(hexA);
  samples[samples.length - 1] = hexToRgb(hexB);
  return fitCosinePaletteToSamples(samples);
};
function accentChain(hexes) {
  if (hexes.length === 0) return accentPair(ACCENTS.indigo, ACCENTS.cyan);
  if (hexes.length === 1) {
    const v = hexToRgb(hexes[0]);
    return { a: v, b: [0, 0, 0], c: [1, 1, 1], d: [0, 0, 0] };
  }
  const anchors = hexes.map((h) => rgbToOklch(hexToRgb(h)));
  const samples = sampleChainRgb(anchors, 17);
  return fitCosinePaletteToSamples(samples);
}
var PALETTES = {
  // cyan → indigo → magenta — the cool iridescent signature
  // magenta → orange → lime → cyan → violet — one full hue cycle across the
  // band, and the default. Lifted so the top channel stays near full all the
  // way round: any lower and yellow turns olive, and the band dirties the page.
  spectrum: { a: [0.74, 0.72, 0.76], b: [0.5, 0.52, 0.48], c: [1, 1, 1], d: [0.92, 0.25, 0.58] },
  prism: accentChain(["#00C0E8", "#6155F5", "#CB30E0"]),
  // pink → magenta
  berry: accentChain(["#FF2D55", "#CB30E0"]),
  // blue → cyan → green
  lagoon: accentChain(["#0088FF", "#00C0E8", "#34C759"]),
  // green → yellow → orange
  citrus: accentChain(["#34C759", "#FFCC00", "#FF8D28"]),
  // cyan → blue → indigo — the blue family
  azure: accentChain(["#00C0E8", "#0088FF", "#6155F5"]),
  // yellow → orange → pink — fire
  ember: accentChain(["#FFCC00", "#FF8D28", "#FF2D55"])
};
function resolvePalette(p) {
  if (!p) return PALETTES.spectrum;
  if (typeof p === "string") return PALETTES[p];
  return p;
}

// src/core/easings.ts
function cubicBezier(x1, y1, x2, y2) {
  const bezX = (t) => 3 * (1 - t) * (1 - t) * t * x1 + 3 * (1 - t) * t * t * x2 + t * t * t;
  const bezY = (t) => 3 * (1 - t) * (1 - t) * t * y1 + 3 * (1 - t) * t * t * y2 + t * t * t;
  const bezXd = (t) => 3 * (1 - 4 * t + 3 * t * t) * x1 + 3 * (2 * t - 3 * t * t) * x2 + 3 * t * t;
  return (x) => {
    if (x <= 0) return 0;
    if (x >= 1) return 1;
    let t = x;
    for (let i = 0; i < 8; i++) {
      const dx = bezX(t) - x;
      if (Math.abs(dx) < 1e-6) break;
      const d = bezXd(t);
      if (Math.abs(d) < 1e-6) break;
      t -= dx / d;
    }
    return bezY(t);
  };
}
var EASINGS = {
  linear: (p) => p,
  easeOutQuart: (p) => 1 - Math.pow(1 - p, 4),
  easeOutCubic: (p) => 1 - Math.pow(1 - p, 3),
  easeInCubic: (p) => p * p * p,
  easeInOutCubic: (p) => p < 0.5 ? 4 * p * p * p : 1 - Math.pow(-2 * p + 2, 3) / 2,
  easeOutExpo: (p) => p === 1 ? 1 : 1 - Math.pow(2, -10 * p),
  easeInOutQuint: (p) => p < 0.5 ? 16 * p * p * p * p * p : 1 - Math.pow(-2 * p + 2, 5) / 2,
  // cubic-bezier(1, 0, 0.35, 0.95) — holds at the start, then whips forward.
  snap: cubicBezier(1, 0, 0.35, 0.95),
  // cubic-bezier(0.25, 0.1, 0.25, 1) — CSS default `ease`. Smooth start, gentle finish.
  ease: cubicBezier(0.25, 0.1, 0.25, 1),
  // cubic-bezier(0.175, 0.885, 0.32, 1.1) — fast accel, overshoots past 1, settles.
  back: cubicBezier(0.175, 0.885, 0.32, 1.1)
};
var resolveEasing = (e) => {
  if (!e) return EASINGS.easeOutQuart;
  if (typeof e === "function") return e;
  return EASINGS[e];
};

// src/core/shader.ts
var VS = `
attribute vec2 a;
void main(){ gl_Position = vec4(a, 0.0, 1.0); }
`;
var FS = `
precision mediump float;
uniform vec2 uRes;
uniform float uFeather; // device px faded at the canvas edges, 0 = off
uniform float uTime;
uniform float uProgress;
uniform float uAlpha;
uniform float uBandTight;
uniform float uPosStart;
uniform float uPosEnd;
uniform float uHueShift;
uniform float uDirection; // 0 = horizontal, 1 = vertical
uniform float uWaveAmount;   // 0..2, multiplies edge wave displacement
uniform float uRippleAmount; // 0..2, multiplies vertical ripple intensity
uniform float uWaveSpeed;    // 0..3, multiplies all time-based motion
uniform float uBrightness;   // 0..1.5, scales the band's RGB before composite
uniform float uSwellAmount;  // 0..1, depth/iridescence intensity. 0 = flat band.
uniform float uCurve;        // how far the band's ends trail its middle, in travel-axis UV
uniform float uSpan;         // 0..1, share of the cross axis the band covers, centred
uniform float uCenter;       // where the curve and span centre on the cross axis, in UV
uniform vec3 uPalA;
uniform vec3 uPalB;
uniform vec3 uPalC;
uniform vec3 uPalD;

#define PI 3.14159265359

vec3 pal(in float t, in vec3 a, in vec3 b, in vec3 c, in vec3 d) {
  return a + b * cos(2.0 * PI * (c * t + d));
}

void main(){
  vec2 uv = gl_FragCoord.xy / uRes;
  float axis  = mix(uv.x, uv.y, uDirection);
  float cross = mix(uv.y, uv.x, uDirection);

  float pos = uPosStart + uProgress * (uPosEnd - uPosStart);

  float tw = uTime * uWaveSpeed;

  // One shallow bend keeps the silhouette organic without reading as a
  // wave. It moves less than 0.5% of the viewport and drifts slowly enough
  // to feel shaped rather than animated; richer motion stays inside the
  // foil texture below instead of distorting the band's sides.
  float waveX = sin(cross * 4.2 + tw * 0.08 + 0.3) * 0.004;
  waveX *= uWaveAmount;

  // Curve: the ends trail the middle on a parabola, reaching uCurve at the
  // edges of the span. Span: the band covers only the middle of the cross
  // axis and fades out towards its ends.
  float c = (cross - uCenter) / max(0.5 * uSpan, 0.001);
  float cc = clamp(c, -1.0, 1.0);
  float lag = uCurve * cc * cc;
  float spanMask = uSpan >= 1.0 ? 1.0 : 1.0 - smoothstep(0.45, 1.0, abs(c));

  float d = (axis - pos) - waveX + sign(uPosEnd - uPosStart) * lag;
  float band = exp(-d * d * uBandTight);

  // Analytic slope of the band's pseudo-elevation map along the travel
  // axis only. We deliberately ignore the cross-axis chain-rule term
  // (\u2202waveX/\u2202cross) \u2014 letting the high-frequency edge wobble leak into
  // the normal made iridescence shimmer at the wave's frequency, which
  // read as "too wavy". Keeping the cross slope at zero gives a clean
  // left\u2192right hue sweep that matches the iOS name-drop feel.
  float dhDaxis = -2.0 * d * uBandTight * band;
  vec2 slope;
  slope.x = mix(dhDaxis, 0.0, uDirection);
  slope.y = mix(0.0, dhDaxis, uDirection);

  // Synthesised surface normal. The 0.18 gain controls perceived
  // height \u2014 higher = steeper flanks, more dramatic iridescent shift.
  vec3 N = normalize(vec3(-slope.x * 0.18, slope.y * 0.18, 1.0));

  // Keep a short, faint wake behind the crest, in the travel direction.
  // The old saturating trail tinted the entire revealed page until fade-out.
  float behind = max(-d * sign(uPosEnd - uPosStart), 0.0);
  float trail = smoothstep(0.0, 0.08, behind)
              * (1.0 - smoothstep(0.12, 0.65, behind)) * 0.10;

  float midpointFocus = 4.0 * uProgress * (1.0 - uProgress);
  float halo = exp(-d * d * 2.5) * 0.05 * midpointFocus;
  float intensity = max(band, trail) * spanMask;
  intensity = clamp(intensity + halo * (1.0 - band), 0.0, 1.0);

  // Keep the crest continuous to the viewport edges. A vertical fade here
  // exposes the hard snapshot boundary at the top and bottom of a wipe.
  float vfade = 1.0;

  // Hue rotates with the synthesised normal \u2014 the trick that reads as
  // iOS-name-drop iridescence \u2014 but on a deliberately gentle scale so
  // the foil shift looks calm, not strobing.
  // Ripple changes the foil texture, not the edge geometry. Keeping those
  // responsibilities separate lets the band stay calm while its colour
  // still feels alive.
  float ripple = sin(cross * 12.0 + axis * 3.0 + tw * 0.40)
               * 0.015 * uRippleAmount;
  float t = N.x * 0.12 + N.y * 0.08
          + axis * 0.90 + cross * 0.16
          + ripple + uHueShift + uTime * 0.04;
  // Low-pass the cosine palette across neighbouring samples. This keeps
  // adjacent hues flowing into one another instead of forming hard colour
  // lanes when the surface normal changes around the crest.
  vec3 col = pal(t, uPalA, uPalB, uPalC, uPalD) * 0.50
           + pal(t - 0.18, uPalA, uPalB, uPalC, uPalD) * 0.25
           + pal(t + 0.18, uPalA, uPalB, uPalC, uPalD) * 0.25;
  col *= uBrightness;

  // Fixed key light + camera looking down +z. View-independent because
  // there's no real camera; this gives a stable highlight that travels
  // across the crest as the band moves, instead of one that wobbles with
  // viewport size.
  vec3 V = vec3(0.0, 0.0, 1.0);
  vec3 L = normalize(vec3(0.35, 0.55, 0.9));
  vec3 H = normalize(L + V);
  float NdotH = clamp(dot(N, H), 0.0, 1.0);
  float NdotV = clamp(dot(N, V), 0.0, 1.0);
  float fresnel = pow(1.0 - NdotV, 3.0);
  float spec    = pow(NdotH, 80.0);

  // Edge fade: as the band's traversal progress nears 0 or 1 (entering or
  // exiting the screen) the band reads at 20% alpha; at midpoint it's at
  // 100%. Softens the band's appearance/disappearance so it doesn't pop
  // into existence at full strength.
  float entryFade = mix(0.2, 1.0, 4.0 * uProgress * (1.0 - uProgress));

  // Body \u2014 palette colour where the band has presence. Premultiplied.
  float bodyA = intensity * vfade * uAlpha * entryFade;
  vec3  bodyPM = col * bodyA;

  // Highlights are emissive \u2014 they add light without occluding the page,
  // gated to the band's body so they only fire on the crest, not the wake.
  float highMask = band * spanMask * vfade * uAlpha * entryFade * uSwellAmount;
  vec3  highEmit = (col * fresnel * 0.55 + vec3(spec) * 1.1) * highMask;
  float highA    = (fresnel * 0.4 + spec * 0.9) * highMask;

  // Fade to nothing at the canvas edges, so a sweep inside a container has

  // no hard border. Off (0) when the canvas fills the viewport.

  vec2 edgeDist = min(gl_FragCoord.xy, uRes - gl_FragCoord.xy);

  float edgeFade = uFeather > 0.0 ? smoothstep(0.0, uFeather, min(edgeDist.x, edgeDist.y)) : 1.0;

  gl_FragColor = vec4(bodyPM + highEmit, min(bodyA + highA, 1.0)) * edgeFade;
}
`;
var containedOverscan = (bandTight) => Math.max(0.2, Math.sqrt(4.6 / Math.max(0.1, bandTight)));
var dirToUniforms = (d, m = 0.2) => {
  switch (d) {
    case "ltr":
      return { axis: 0, posStart: -m, posEnd: 1 + m };
    case "rtl":
      return { axis: 0, posStart: 1 + m, posEnd: -m };
    case "ttb":
      return { axis: 1, posStart: 1 + m, posEnd: -m };
    case "btt":
      return { axis: 1, posStart: -m, posEnd: 1 + m };
  }
};
function createShader(opts = {}) {
  if (typeof window === "undefined") return null;
  const canvas = opts.canvas ?? document.createElement("canvas");
  const gl = canvas.getContext("webgl", {
    alpha: true,
    antialias: false,
    premultipliedAlpha: true
  });
  if (!gl) return null;
  const compile = (type, src) => {
    const s = gl.createShader(type);
    if (!s) return null;
    gl.shaderSource(s, src);
    gl.compileShader(s);
    if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) {
      console.error("[glimm] shader compile error:", gl.getShaderInfoLog(s));
      gl.deleteShader(s);
      return null;
    }
    return s;
  };
  const vs = compile(gl.VERTEX_SHADER, VS);
  if (!vs) return null;
  const fs = compile(gl.FRAGMENT_SHADER, FS);
  if (!fs) {
    gl.deleteShader(vs);
    return null;
  }
  const prog = gl.createProgram();
  if (!prog) {
    gl.deleteShader(vs);
    gl.deleteShader(fs);
    return null;
  }
  gl.attachShader(prog, vs);
  gl.attachShader(prog, fs);
  gl.linkProgram(prog);
  if (!gl.getProgramParameter(prog, gl.LINK_STATUS)) {
    console.error("[glimm] shader link error:", gl.getProgramInfoLog(prog));
    gl.deleteProgram(prog);
    gl.deleteShader(vs);
    gl.deleteShader(fs);
    return null;
  }
  gl.useProgram(prog);
  const buf = gl.createBuffer();
  gl.bindBuffer(gl.ARRAY_BUFFER, buf);
  gl.bufferData(
    gl.ARRAY_BUFFER,
    new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]),
    gl.STATIC_DRAW
  );
  const aLoc = gl.getAttribLocation(prog, "a");
  gl.enableVertexAttribArray(aLoc);
  gl.vertexAttribPointer(aLoc, 2, gl.FLOAT, false, 0, 0);
  const u = (n) => gl.getUniformLocation(prog, n);
  const uRes = u("uRes");
  const uFeather = u("uFeather");
  const uTime = u("uTime");
  const uProgress = u("uProgress");
  const uAlpha = u("uAlpha");
  const uBandTight = u("uBandTight");
  const uPosStart = u("uPosStart");
  const uPosEnd = u("uPosEnd");
  const uHueShift = u("uHueShift");
  const uDirection = u("uDirection");
  const uWaveAmount = u("uWaveAmount");
  const uRippleAmount = u("uRippleAmount");
  const uWaveSpeed = u("uWaveSpeed");
  const uBrightness = u("uBrightness");
  const uSwellAmount = u("uSwellAmount");
  const uCurve = u("uCurve");
  const uSpan = u("uSpan");
  const uCenter = u("uCenter");
  const uPalA = u("uPalA");
  const uPalB = u("uPalB");
  const uPalC = u("uPalC");
  const uPalD = u("uPalD");
  gl.enable(gl.BLEND);
  gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
  const clamp = (v, min, max) => Math.max(min, Math.min(max, v));
  const state = {
    progress: 0,
    alpha: 0,
    palette: opts.palette ?? resolvePalette(void 0),
    bandTight: clamp(opts.bandTight ?? 14, 0.1, 200),
    direction: opts.direction ?? "ltr",
    waveAmount: clamp(opts.waveAmount ?? 0, 0, 2),
    rippleAmount: clamp(opts.rippleAmount ?? 1, 0, 2),
    waveSpeed: clamp(opts.waveSpeed ?? 1, 0, 3),
    brightness: clamp(opts.brightness ?? 1, 0, 1.5),
    swellAmount: clamp(opts.swellAmount ?? 0.55, 0, 1),
    curve: opts.curve ?? 0,
    span: clamp(opts.span ?? 1, 0.05, 1),
    center: opts.center ?? 0.5
  };
  const hueShift = Math.random() * 0.4;
  let feather = opts.feather ?? "auto";
  let featherPx = 0;
  let contained = false;
  const updateFeather = () => {
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const r = canvas.getBoundingClientRect();
    const fullBleed = r.width >= window.innerWidth - 1 && r.height >= window.innerHeight - 1;
    contained = !fullBleed;
    if (feather === "auto") {
      featherPx = fullBleed ? 0 : Math.min(24, 0.15 * Math.min(r.width, r.height)) * dpr;
    } else {
      featherPx = Math.max(0, feather) * dpr;
    }
  };
  const resize = () => {
    updateFeather();
    const dpr = Math.min(window.devicePixelRatio || 1, 2);
    const r = canvas.getBoundingClientRect();
    const w = Math.max(1, Math.round(r.width * dpr));
    const h = Math.max(1, Math.round(r.height * dpr));
    if (canvas.width !== w || canvas.height !== h) {
      canvas.width = w;
      canvas.height = h;
      gl.viewport(0, 0, w, h);
    }
  };
  resize();
  const ro = new ResizeObserver(resize);
  ro.observe(canvas);
  const start = performance.now();
  let raf = 0;
  let destroyed = false;
  const tick = () => {
    raf = 0;
    if (destroyed || state.alpha === 0) return;
    const t = (performance.now() - start) / 1e3;
    const dirU = dirToUniforms(state.direction, contained ? containedOverscan(state.bandTight) : 0.2);
    gl.clearColor(0, 0, 0, 0);
    gl.clear(gl.COLOR_BUFFER_BIT);
    gl.uniform2f(uRes, canvas.width, canvas.height);
    gl.uniform1f(uFeather, featherPx);
    gl.uniform1f(uTime, t);
    gl.uniform1f(uProgress, state.progress);
    gl.uniform1f(uAlpha, state.alpha);
    gl.uniform1f(uBandTight, state.bandTight);
    gl.uniform1f(uPosStart, dirU.posStart);
    gl.uniform1f(uPosEnd, dirU.posEnd);
    gl.uniform1f(uDirection, dirU.axis);
    gl.uniform1f(uWaveAmount, state.waveAmount);
    gl.uniform1f(uRippleAmount, state.rippleAmount);
    gl.uniform1f(uWaveSpeed, state.waveSpeed);
    gl.uniform1f(uBrightness, state.brightness);
    gl.uniform1f(uSwellAmount, state.swellAmount);
    gl.uniform1f(uCurve, state.curve);
    gl.uniform1f(uSpan, state.span);
    gl.uniform1f(uCenter, state.center);
    gl.uniform1f(uHueShift, hueShift);
    gl.uniform3f(uPalA, state.palette.a[0], state.palette.a[1], state.palette.a[2]);
    gl.uniform3f(uPalB, state.palette.b[0], state.palette.b[1], state.palette.b[2]);
    gl.uniform3f(uPalC, state.palette.c[0], state.palette.c[1], state.palette.c[2]);
    gl.uniform3f(uPalD, state.palette.d[0], state.palette.d[1], state.palette.d[2]);
    gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    raf = requestAnimationFrame(tick);
  };
  return {
    canvas,
    setProgress: (p) => {
      state.progress = clamp(p, 0, 1);
    },
    setAlpha: (a) => {
      if (destroyed) return;
      state.alpha = clamp(a, 0, 1.5);
      if (state.alpha > 0) {
        if (!raf) raf = requestAnimationFrame(tick);
      } else {
        cancelAnimationFrame(raf);
        raf = 0;
        gl.clearColor(0, 0, 0, 0);
        gl.clear(gl.COLOR_BUFFER_BIT);
      }
    },
    setPalette: (p) => {
      state.palette = p;
    },
    setBandTight: (b) => {
      state.bandTight = clamp(b, 0.1, 200);
    },
    setDirection: (d) => {
      state.direction = d;
    },
    setWaveAmount: (v) => {
      state.waveAmount = clamp(v, 0, 2);
    },
    setRippleAmount: (v) => {
      state.rippleAmount = clamp(v, 0, 2);
    },
    setWaveSpeed: (v) => {
      state.waveSpeed = clamp(v, 0, 3);
    },
    setBrightness: (v) => {
      state.brightness = clamp(v, 0, 1.5);
    },
    setSwellAmount: (v) => {
      state.swellAmount = clamp(v, 0, 1);
    },
    setCenter: (v) => {
      state.center = v;
    },
    setFeather: (v) => {
      feather = v;
      updateFeather();
    },
    getProgress: () => state.progress,
    getAlpha: () => state.alpha,
    destroy: () => {
      if (destroyed) return;
      destroyed = true;
      cancelAnimationFrame(raf);
      ro.disconnect();
      gl.deleteProgram(prog);
      gl.deleteBuffer(buf);
      gl.deleteShader(vs);
      gl.deleteShader(fs);
    }
  };
}

// src/core/sweep.ts
var easeOutQuart = (p) => 1 - Math.pow(1 - p, 4);
var easeInOutCubic = (p) => p < 0.5 ? 4 * p * p * p : 1 - Math.pow(-2 * p + 2, 3) / 2;
function playSweep(ctrl, opts = {}) {
  const sweepMs = finite(opts.sweepMs, 1100, 0);
  const outroMs = finite(opts.outroMs, 220, 0);
  const midpoint = finite(opts.midpoint, 0.56, 0, 1);
  const easing = resolveEasing(opts.easing ?? "ease") ?? easeOutQuart;
  const peakAlpha = finite(opts.peakAlpha, 1, 0, 1.5);
  if (opts.palette) ctrl.setPalette(resolvePalette(opts.palette));
  if (opts.bandTight !== void 0) ctrl.setBandTight(opts.bandTight);
  if (opts.direction) ctrl.setDirection(opts.direction);
  if (opts.waveAmount !== void 0) ctrl.setWaveAmount(opts.waveAmount);
  if (opts.rippleAmount !== void 0) ctrl.setRippleAmount(opts.rippleAmount);
  if (opts.waveSpeed !== void 0) ctrl.setWaveSpeed(opts.waveSpeed);
  if (opts.brightness !== void 0) ctrl.setBrightness(opts.brightness);
  if (opts.swellAmount !== void 0) ctrl.setSwellAmount(opts.swellAmount);
  let cancelled = false;
  let raf = 0;
  let resolvePhase;
  let resolveMidpoint;
  let resolveDone;
  let resolveCancelled;
  const cancelledP = new Promise((resolve) => {
    resolveCancelled = resolve;
  });
  const midpointP = new Promise((r) => {
    resolveMidpoint = r;
  });
  const doneP = new Promise((r) => {
    resolveDone = r;
  });
  const animate = (duration, update) => new Promise((resolve, reject) => {
    resolvePhase = resolve;
    const start = performance.now();
    const tick = () => {
      if (cancelled) {
        resolve();
        return;
      }
      try {
        const raw = duration === 0 ? 1 : Math.min(1, (performance.now() - start) / duration);
        update(raw);
        if (cancelled || raw === 1) resolve();
        else raf = requestAnimationFrame(tick);
      } catch (error) {
        reject(error);
      }
    };
    raf = requestAnimationFrame(tick);
  });
  (async () => {
    const currentProgress = ctrl.getProgress();
    const startProgress = currentProgress >= 0.999 ? 0 : Math.max(0, Math.min(1, currentProgress));
    ctrl.setAlpha(peakAlpha);
    ctrl.setProgress(startProgress);
    const remaining = 1 - startProgress;
    const phaseMs = Math.max(80, sweepMs * remaining);
    let midpointFired = false;
    let midpointTask = Promise.resolve();
    const fireMidpoint = () => {
      midpointFired = true;
      midpointTask = Promise.resolve().then(() => {
        if (!cancelled) return opts.onMidpoint?.();
      }).then(() => {
      });
      midpointTask.then(resolveMidpoint, (error) => {
        console.error("[glimm] midpoint callback failed:", error);
        resolveMidpoint();
      });
    };
    await animate(phaseMs, (raw) => {
      const eased = finite(easing(raw), raw, 0, 1);
      const progress = startProgress + remaining * eased;
      ctrl.setProgress(progress);
      if (!midpointFired && progress >= midpoint) fireMidpoint();
    });
    if (cancelled) return;
    if (!midpointFired) {
      fireMidpoint();
    }
    await Promise.race([midpointTask.catch(() => {
    }), cancelledP]);
    if (cancelled) return;
    await animate(outroMs, (raw) => ctrl.setAlpha(peakAlpha * (1 - easeInOutCubic(raw))));
    if (cancelled) return;
    ctrl.setProgress(0);
    opts.onComplete?.();
    resolveDone();
  })().catch((error) => {
    console.error("[glimm] sweep callback failed:", error);
    if (!cancelled) {
      ctrl.setAlpha(0);
      ctrl.setProgress(0);
    }
  }).finally(() => {
    resolveMidpoint();
    resolveDone();
  });
  return {
    midpoint: midpointP,
    done: doneP,
    cancel: () => {
      cancelled = true;
      cancelAnimationFrame(raf);
      resolvePhase?.();
      resolveCancelled();
      resolveMidpoint();
      resolveDone();
    }
  };
}
function finite(value, fallback, min, max = Infinity) {
  return Number.isFinite(value) ? Math.max(min, Math.min(max, value)) : fallback;
}

// src/react/covered-sweep.ts
function createCurtain(host) {
  const element = document.createElement("div");
  element.dataset.glimmCurtain = "idle";
  element.setAttribute("aria-hidden", "true");
  Object.assign(element.style, {
    position: "absolute",
    inset: "0",
    display: "none",
    pointerEvents: "none"
  });
  host.prepend(element);
  let direction = "ltr";
  let progress = 0;
  let phase = "idle";
  const update = (p, nextPhase) => {
    progress = p;
    phase = nextPhase;
    element.dataset.glimmCurtain = phase;
    const edge = (-0.2 + 1.4 * p) * 100;
    const axis = { ltr: "right", rtl: "left", ttb: "bottom", btt: "top" }[direction];
    const before = phase === "cover" ? "#000" : "transparent";
    const after = phase === "cover" ? "transparent" : "#000";
    const mask = `linear-gradient(to ${axis}, ${before} ${edge - 3}%, ${after} ${edge + 3}%)`;
    element.style.maskImage = mask;
    element.style.webkitMaskImage = mask;
  };
  const hide = () => {
    phase = "idle";
    progress = 0;
    element.dataset.glimmCurtain = phase;
    element.style.display = "none";
  };
  return {
    element,
    begin(nextDirection) {
      const start = phase === "cover" && direction === nextDirection ? progress : 0;
      const wasCovered = phase === "hold" || phase === "reveal";
      direction = nextDirection;
      element.style.background = surfaceColor(host);
      element.style.display = "block";
      update(wasCovered ? 1 : start, "cover");
      return wasCovered ? 1 : start;
    },
    update,
    hold() {
      phase = "hold";
      progress = 1;
      element.dataset.glimmCurtain = phase;
      element.style.maskImage = "none";
      element.style.webkitMaskImage = "none";
    },
    hide,
    destroy() {
      element.remove();
    }
  };
}
function surfaceColor(host) {
  for (let node = host.parentElement; node; node = node.parentElement) {
    const color = getComputedStyle(node).backgroundColor;
    if (!color || color === "transparent") continue;
    const alpha = color.startsWith("rgba(") || color.includes("/") ? color.match(/(?:,|\/)\s*([\d.]+%?)\s*\)$/)?.[1] : void 0;
    if (alpha && parseFloat(alpha) < (alpha.endsWith("%") ? 100 : 1)) continue;
    return color;
  }
  return getComputedStyle(document.documentElement).colorScheme === "dark" ? "#111" : "#fff";
}
var HOLD_MS = 200;
function playCoveredSweep(ctrl, curtain, commit, opts) {
  const abort = new AbortController();
  let active;
  let cancelled = false;
  let resolveMidpoint;
  let resolveDone;
  const midpoint = new Promise((resolve) => {
    resolveMidpoint = resolve;
  });
  const done = new Promise((resolve) => {
    resolveDone = resolve;
  });
  const duration = Number.isFinite(opts.sweepMs) ? Math.max(0, opts.sweepMs) : 1100;
  const split = Number.isFinite(opts.midpoint) ? Math.max(0.1, Math.min(0.9, opts.midpoint)) : 0.56;
  const start = curtain.begin(opts.direction ?? "ltr");
  let locked = false;
  let late = false;
  const cancel = (preserve) => {
    cancelled = true;
    abort.abort();
    active?.cancel();
    if (!preserve) {
      curtain.hide();
      ctrl.setAlpha(0);
      ctrl.setProgress(0);
    }
    resolveMidpoint();
    resolveDone();
  };
  void (async () => {
    const coverCtrl = {
      ...ctrl,
      getProgress: () => 0,
      setProgress: (p) => {
        if (!locked) {
          curtain.update(start + (1 - start) * p, "cover");
          ctrl.setProgress(start + (1 - start) * p);
        }
      }
    };
    active = playSweep(coverCtrl, {
      ...opts,
      sweepMs: start >= 1 ? 0 : duration * split,
      outroMs: 0,
      midpoint: 1,
      onComplete: void 0,
      onMidpoint: async () => {
        locked = true;
        curtain.hold();
        ctrl.setAlpha(0);
        await nextPaint(abort.signal);
        if (cancelled) return;
        let timer;
        let holdTimer;
        try {
          const work = waitForWork(commit(abort.signal), abort.signal);
          const inTime = await Promise.race([
            work.then(() => true),
            new Promise((resolve) => {
              holdTimer = setTimeout(() => resolve(false), HOLD_MS);
            })
          ]);
          if (!inTime && !cancelled) {
            late = true;
            curtain.hide();
          }
          await Promise.race([
            work,
            new Promise((resolve) => {
              timer = setTimeout(() => {
                console.warn("[glimm] content commit timed out; releasing the curtain");
                abort.abort();
                resolve();
              }, 8e3);
            })
          ]);
        } catch (error) {
          console.error("[glimm] content commit failed:", error);
        } finally {
          clearTimeout(timer);
          clearTimeout(holdTimer);
        }
        if (!cancelled) await nextPaint(abort.signal);
        resolveMidpoint();
      }
    });
    await active.done;
    if (cancelled) return;
    const revealCtrl = {
      ...ctrl,
      getProgress: () => 0,
      setProgress: (p) => {
        if (!late) curtain.update(p, "reveal");
        ctrl.setProgress(p);
      }
    };
    active = playSweep(revealCtrl, {
      ...opts,
      sweepMs: duration * (1 - split),
      midpoint: 1,
      onMidpoint: () => curtain.hide(),
      onComplete: void 0
    });
    await active.done;
    if (!cancelled) {
      curtain.hide();
      opts.onComplete?.();
    }
  })().catch((error) => {
    console.error("[glimm] covered sweep failed:", error);
    if (!cancelled) {
      curtain.hide();
      ctrl.setAlpha(0);
    }
  }).finally(() => {
    resolveMidpoint();
    resolveDone();
  });
  return { midpoint, done, cancel: () => cancel(false), interrupt: () => cancel(true) };
}
function nextPaint(signal) {
  return new Promise((resolve) => {
    if (signal.aborted) {
      resolve();
      return;
    }
    let frame = 0;
    const finish = () => {
      cancelAnimationFrame(frame);
      signal.removeEventListener("abort", finish);
      resolve();
    };
    signal.addEventListener("abort", finish, { once: true });
    frame = requestAnimationFrame(() => {
      frame = requestAnimationFrame(finish);
    });
  });
}
function waitForWork(work, signal) {
  return new Promise((resolve, reject) => {
    const finish = () => {
      signal.removeEventListener("abort", finish);
      resolve();
    };
    signal.addEventListener("abort", finish, { once: true });
    void work.then(finish, (error) => {
      signal.removeEventListener("abort", finish);
      reject(error);
    });
    if (signal.aborted) finish();
  });
}

// src/react/masked-sweep.ts
var activeCancel;
var FREEZE_MS = 200;
var INPUT = ["pointerdown", "wheel", "keydown"];
function retarget(press) {
  const onClick = (event) => {
    stop();
    if (event.target !== document.documentElement) return;
    const target = document.elementFromPoint(press.clientX, press.clientY);
    if (!(target instanceof HTMLElement) || target === document.documentElement) return;
    event.stopImmediatePropagation();
    const focusable = target.closest('a[href], button, input, select, textarea, summary, [tabindex], [contenteditable=""], [contenteditable="true"]');
    focusable?.focus({ preventScroll: true });
    target.click();
  };
  const timer = setTimeout(() => stop(), 1500);
  const stop = () => {
    clearTimeout(timer);
    removeEventListener("click", onClick, true);
  };
  addEventListener("click", onClick, true);
}
function playMaskedSweep(ctrl, curtain, commit, opts) {
  activeCancel?.();
  const abort = new AbortController();
  const root = document.documentElement;
  const host = curtain.element.parentElement;
  let cancelled = false;
  let transition;
  let animation;
  let resolveMidpoint;
  let resolveDone;
  const midpoint = new Promise((resolve) => {
    resolveMidpoint = resolve;
  });
  const done = new Promise((resolve) => {
    resolveDone = resolve;
  });
  let commitTask;
  const commitOnce = (signal) => commitTask ?? (commitTask = commit(signal));
  const style = document.createElement("style");
  style.textContent = `
    html[data-glimm-transition] { view-transition-name: root !important; }
    html[data-glimm-transition] * { view-transition-name: none !important; }
    html[data-glimm-transition] [data-glimm-layer] { view-transition-name: glimm-layer !important; }
    @keyframes glimm-snapshot-hold { from { opacity: 1 } to { opacity: 1 } }
    html[data-glimm-transition]::view-transition { pointer-events: none; }
    html[data-glimm-transition]::view-transition-group(*) {
      animation: glimm-snapshot-hold 86400s linear both;
    }
    html[data-glimm-transition]::view-transition-image-pair(*) { isolation: auto; }
    html[data-glimm-transition]::view-transition-old(*),
    html[data-glimm-transition]::view-transition-new(*) {
      animation: none; mix-blend-mode: normal; opacity: 1;
    }
    html[data-glimm-transition]::view-transition-old(root) { z-index: 2; }
    html[data-glimm-transition]::view-transition-new(root) {
      z-index: 1;
    }
    html[data-glimm-transition]::view-transition-group(glimm-layer) { z-index: 2147483647; }
    html[data-glimm-transition]::view-transition-old(glimm-layer) { display: none; }
  `;
  let clipAnimation;
  const updateClip = (p) => {
    if (!clipAnimation) {
      const side = { ltr: [0, 0, 0, 1], rtl: [0, 1, 0, 0], ttb: [1, 0, 0, 0], btt: [0, 0, 1, 0] }[opts.direction ?? "ltr"];
      const inset = (k) => `inset(${side.map((on) => on * k + "%").join(" ")})`;
      try {
        clipAnimation = root.animate({ clipPath: [inset(0), inset(100)] }, {
          duration: 1e3,
          fill: "both",
          easing: "linear",
          pseudoElement: "::view-transition-old(root)"
        });
        clipAnimation.pause();
      } catch {
        return;
      }
    }
    clipAnimation.currentTime = Math.max(0, Math.min(1, -0.2 + p * 1.4)) * 1e3;
  };
  const stopInput = () => INPUT.forEach((type) => removeEventListener(type, release, true));
  const cleanup = () => {
    style.remove();
    stopInput();
    clipAnimation?.cancel();
    if (activeCancel !== cancel) return;
    activeCancel = void 0;
    root.removeAttribute("data-glimm-transition");
    host.removeAttribute("data-glimm-layer");
  };
  let released = false;
  const release = (event) => {
    if (released || cancelled) return;
    released = true;
    stopInput();
    transition?.skipTransition();
    if (event instanceof PointerEvent && event.isPrimary && event.button === 0) retarget(event);
  };
  const cancel = () => {
    if (cancelled) return;
    cancelled = true;
    abort.abort();
    animation?.cancel();
    transition?.skipTransition();
    ctrl.setAlpha(0);
    curtain.hide();
    cleanup();
    resolveMidpoint();
    resolveDone();
  };
  activeCancel = cancel;
  curtain.hide();
  ctrl.setAlpha(0);
  ctrl.setProgress(0);
  root.setAttribute("data-glimm-transition", "");
  host.setAttribute("data-glimm-layer", "");
  document.head.append(style);
  void (async () => {
    let timer;
    let late;
    try {
      transition = document.startViewTransition(async () => {
        if (cancelled) return;
        const work = waitForWork(commitOnce(abort.signal), abort.signal);
        const inTime = await Promise.race([
          work.then(() => true),
          new Promise((resolve) => {
            timer = setTimeout(() => resolve(false), FREEZE_MS);
          })
        ]);
        clearTimeout(timer);
        if (!inTime) {
          late = work;
          work.catch(() => {
          });
        }
      });
      void transition.updateCallbackDone.catch(() => {
      });
      void transition.finished.catch(() => {
      });
      await transition.ready;
    } catch (error) {
      clearTimeout(timer);
      transition?.skipTransition();
      if (cancelled) return;
      cleanup();
      animation = playCoveredSweep(ctrl, curtain, commitOnce, opts);
      void animation.midpoint.then(resolveMidpoint);
      await animation.done;
      return;
    }
    if (cancelled) return;
    if (late) {
      transition.skipTransition();
      cleanup();
      activeCancel = cancel;
      try {
        await Promise.race([
          late,
          new Promise((resolve) => {
            timer = setTimeout(() => {
              console.warn("[glimm] content commit timed out; skipping the sweep");
              cancel();
              resolve();
            }, 8e3);
          })
        ]);
      } finally {
        clearTimeout(timer);
      }
      if (cancelled) return;
      await nextPaint(abort.signal);
      if (cancelled) return;
      animation = playSweep(ctrl, opts);
      void animation.midpoint.then(resolveMidpoint);
      await animation.done;
      if (activeCancel === cancel) activeCancel = void 0;
      return;
    }
    INPUT.forEach((type) => addEventListener(type, release, { capture: true, passive: true }));
    updateClip(0);
    let finishing = false;
    void transition.finished.then(() => {
      if (!finishing && !cancelled && !released) cancel();
    });
    const maskedCtrl = {
      ...ctrl,
      setProgress: (p) => {
        updateClip(p);
        ctrl.setProgress(p);
      }
    };
    animation = playSweep(maskedCtrl, { ...opts, onComplete: void 0 });
    void animation.midpoint.then(resolveMidpoint);
    await animation.done;
    if (cancelled) return;
    updateClip(1);
    finishing = true;
    transition.skipTransition();
    await transition.finished;
    if (!cancelled) {
      cleanup();
      opts.onComplete?.();
    }
  })().catch((error) => {
    console.error("[glimm] masked sweep failed:", error);
    cancel();
  }).finally(() => {
    resolveMidpoint();
    resolveDone();
  });
  return { midpoint, done, cancel, interrupt: cancel };
}
var GlimmContext = createContext(null);
function useGlimm() {
  const ctx = useContext(GlimmContext);
  if (!ctx) {
    throw new Error("useGlimm must be used inside <GlimmProvider>");
  }
  return ctx;
}
var prefersReducedMotion = () => typeof window !== "undefined" && window.matchMedia?.("(prefers-reduced-motion: reduce)").matches;
function GlimmProvider({
  children,
  ...defaults
}) {
  const canvasHostRef = useRef(null);
  const ctrlRef = useRef(null);
  const activeRef = useRef(null);
  const curtainRef = useRef(null);
  const [isPending, startTransition] = useTransition();
  const [committedTicket, setCommittedTicket] = useState(0);
  const ticketRef = useRef(0);
  const waitersRef = useRef(/* @__PURE__ */ new Map());
  useEffect(() => {
    if (isPending) return;
    for (const [ticket, finish] of waitersRef.current) {
      if (ticket <= committedTicket) finish();
    }
  }, [committedTicket, isPending]);
  const commit = (navigate, signal) => {
    if (signal.aborted) return Promise.resolve();
    const ticket = ++ticketRef.current;
    let result;
    let finish;
    const committed = new Promise((resolve) => {
      finish = () => {
        waitersRef.current.delete(ticket);
        signal.removeEventListener("abort", finish);
        resolve();
      };
      waitersRef.current.set(ticket, finish);
      signal.addEventListener("abort", finish, { once: true });
    });
    try {
      startTransition(() => {
        result = navigate();
        setCommittedTicket(ticket);
      });
    } catch (error) {
      finish();
      return Promise.reject(error);
    }
    return Promise.all([committed, result]).then(() => {
    });
  };
  const defaultsRef = useRef(defaults);
  useEffect(() => {
    defaultsRef.current = defaults;
  }, [defaults]);
  const ensureController = () => {
    if (ctrlRef.current) return ctrlRef.current;
    const host = canvasHostRef.current;
    if (!host) return null;
    const canvas = document.createElement("canvas");
    canvas.setAttribute("aria-hidden", "true");
    Object.assign(canvas.style, {
      position: "absolute",
      inset: "0",
      width: "100%",
      height: "100%",
      display: "block",
      pointerEvents: "none"
    });
    host.appendChild(canvas);
    const currentDefaults = defaultsRef.current;
    const factory = currentDefaults.shaderFactory ?? createShader;
    const ctrl = factory({
      canvas,
      palette: resolvePalette(currentDefaults.palette),
      bandTight: currentDefaults.bandTight,
      direction: currentDefaults.direction
    });
    if (!ctrl) {
      canvas.remove();
      return null;
    }
    curtainRef.current = createCurtain(host);
    ctrlRef.current = ctrl;
    currentDefaults.onController?.(ctrl);
    return ctrl;
  };
  useEffect(() => {
    return () => {
      activeRef.current?.cancel();
      curtainRef.current?.destroy();
      curtainRef.current = null;
      for (const finish of waitersRef.current.values()) finish();
      ctrlRef.current?.destroy();
      ctrlRef.current?.canvas.remove();
      ctrlRef.current = null;
    };
  }, []);
  const sweep = useMemo(() => {
    return (navigate, options) => {
      const d = defaultsRef.current;
      if (activeRef.current?.interrupt) activeRef.current.interrupt();
      else activeRef.current?.cancel();
      activeRef.current = null;
      const instant = () => {
        curtainRef.current?.hide();
        ctrlRef.current?.setAlpha(0);
        ctrlRef.current?.setProgress(0);
        let cancelled = false;
        let resolveCancelled;
        const cancelledP = new Promise((resolve) => {
          resolveCancelled = resolve;
        });
        const task = Promise.resolve().then(async () => {
          if (cancelled) return;
          await navigate();
          if (cancelled) return;
          await options?.onMidpoint?.();
          if (!cancelled) options?.onComplete?.();
        }).catch((error) => {
          console.error("[glimm] navigation callback failed:", error);
        });
        const p = Promise.race([task, cancelledP]);
        const handle2 = { midpoint: p, done: p, cancel: () => {
          cancelled = true;
          resolveCancelled();
        } };
        activeRef.current = handle2;
        void p.then(() => {
          if (activeRef.current === handle2) activeRef.current = null;
        });
        return handle2;
      };
      if (d.reducedMotion !== "sweep" && prefersReducedMotion()) {
        return instant();
      }
      const ctrl = ensureController();
      if (!ctrl) return instant();
      const play = typeof document.startViewTransition === "function" ? playMaskedSweep : playCoveredSweep;
      const handle = play(ctrl, curtainRef.current, async (signal) => {
        await commit(navigate, signal);
        if (!signal.aborted) await options?.onMidpoint?.();
      }, { ...d, ...options, onMidpoint: void 0 });
      activeRef.current = handle;
      handle.done.finally(() => {
        if (activeRef.current === handle) activeRef.current = null;
      });
      return handle;
    };
  }, []);
  const value = useMemo(
    () => ({ sweep, defaults }),
    [sweep, defaults]
  );
  return /* @__PURE__ */ jsxs(GlimmContext.Provider, { value, children: [
    children,
    /* @__PURE__ */ jsx(
      "div",
      {
        ref: canvasHostRef,
        "aria-hidden": true,
        style: {
          position: "fixed",
          inset: 0,
          pointerEvents: "none",
          zIndex: defaults.zIndex ?? 9999
        }
      }
    )
  ] });
}
var KEY = "glimm:page-effect";
var EVENT = "glimm:page-effect";
function getPageEffect() {
  try {
    return localStorage.getItem(KEY) !== "off";
  } catch {
    return true;
  }
}
function setPageEffect(on) {
  try {
    if (on) localStorage.removeItem(KEY);
    else localStorage.setItem(KEY, "off");
  } catch {
  }
  window.dispatchEvent(new Event(EVENT));
}
function subscribe(fn) {
  window.addEventListener(EVENT, fn);
  window.addEventListener("storage", fn);
  return () => {
    window.removeEventListener(EVENT, fn);
    window.removeEventListener("storage", fn);
  };
}
function usePageEffect() {
  const on = useSyncExternalStore(subscribe, getPageEffect, () => true);
  return [on, setPageEffect];
}
function supportsHtmlInCanvas() {
  if (typeof document === "undefined") return false;
  const canvas = document.createElement("canvas");
  canvas.setAttribute("content", "drawable");
  return canvas.content === "drawable";
}
var RELEASE = 220;
var BURST_AT = 90;
var PEEK = 0.08;
var INSTANT = 120;
var SNAP = 440;
var HOLD = 0.32;
var HOLD_AFTER = 400;
var RELAX = 700;
var LET_GO = 320;
var RAMP = 140;
var FAST = 160;
var ORIGIN_MS = 160;
var RING_IN = 120;
var VS2 = `#version 300 es
in vec2 aPos;
out vec2 vUV;
void main() {
  vUV = aPos * 0.5 + 0.5;
  vUV.y = 1.0 - vUV.y;
  gl_Position = vec4(aPos, 0.0, 1.0);
}`;
var FS2 = `#version 300 es
precision highp float;
in vec2 vUV;
out vec4 outColor;
uniform sampler2D uNew;
uniform sampler2D uOld;
uniform vec2  uRes;     // CSS px
uniform float uDpr;
uniform vec2  uO;       // origin, CSS px
uniform vec2  uDir;     // the band's travel, a unit vector
uniform float uAxis;    // 0 runs up or down the box, 1 runs across it
uniform float uScale;   // 0..1, how hard the content moves, by box size
uniform float uDim;     // 0..1, how much the rest dims while gathering
uniform float uGather;  // 0..1, pull towards the origin
uniform float uHaze;    // 0..1, how much the gather softens (none in a long hold)
uniform float uFront;   // the band's crest, CSS px along the travel axis
uniform float uWidth;   // crest half-width, CSS px
uniform float uRing;    // 0..1, ripple strength (fades as it travels)
uniform float uCurve;   // the band's curve, as a fraction of the travel length
uniform float uSpan;    // 0..1, share of the cross axis the band covers
uniform float uCenter;  // where the curve and span centre on the cross axis, 0..1
uniform float uBurst;   // 1 once the run has started (old/new split on)
uniform float uActive;

vec4 page(vec2 p, bool isNew, float lod) {
  vec2 uv = clamp(p / uRes, vec2(0.0), vec2(1.0));
  return isNew ? textureLod(uNew, uv, lod) : textureLod(uOld, uv, lod);
}

vec4 soft(vec2 p, bool isNew, float r) {
  if (r < 0.4) return page(p, isNew, 0.0);
  float lod = max(0.0, log2(r * uDpr / 5.0));
  vec4 acc = page(p, isNew, lod);
  for (int i = 1; i < 16; i++) {
    float fi = float(i);
    float a = fi * 2.39996;
    acc += page(p + vec2(cos(a), sin(a)) * sqrt(fi / 16.0) * r, isNew, lod);
  }
  return acc / 16.0;
}

void main() {
  if (uActive < 0.5) { outColor = texture(uNew, vUV); return; }
  vec2 px = vUV * uRes;
  vec2 d = px - uO;
  float dist = length(d);
  float span = min(uRes.x, uRes.y);

  // Gather: sample further out near the origin, so the content there is
  // drawn in towards it. The softening rings it: the origin is usually what
  // was just pressed, so that stays readable, and the blur comes in around it.
  float near = exp(-dist / (0.45 * span));
  vec2 sp = px + d * uGather * 0.16 * near;
  float blur = uHaze * 16.0 * uScale * near * near * smoothstep(48.0, 48.0 + 0.22 * span, dist);

  // Along and across the band's travel
  vec2 dir = uDir;
  float L = uAxis > 0.5 ? uRes.x : uRes.y;
  // Distance travelled from the edge the band enters by
  float al = dot(px, dir) + (dir.x + dir.y < 0.0 ? L : 0.0);
  float ac = uAxis > 0.5 ? px.y : px.x;
  float A = uAxis > 0.5 ? uRes.y : uRes.x;

  // The crest pushes the content on as it passes and a weaker trough
  // behind it pulls back, so the surface ripples and springs flat.
  // Follows the band's curve and span: the ends trail the middle, and the
  // effect fades out towards the ends of the span.
  float c = (ac / A - uCenter) / (0.5 * uSpan);
  float cc = clamp(c, -1.0, 1.0);
  float spanMask = uSpan >= 1.0 ? 1.0 : 1.0 - smoothstep(0.45, 1.0, abs(c));
  float lag = uCurve * cc * cc * L;
  float front = uFront - lag;
  float u = (al - front) / uWidth;
  float crest = exp(-u * u);
  float trough = exp(-pow((u + 2.6) / 1.4, 2.0));
  float push = (crest * 22.0 - trough * 8.0) * uRing * spanMask * uScale;
  sp -= dir * push;
  // Our progressive blur: heaviest just behind the crest, easing out ahead
  // of it and over a longer run behind, as in the band.
  float bu = u * 0.5;
  blur += 14.0 * uScale * uRing * spanMask * exp(-pow((bu + 0.2) / 0.9, 2.0));

  // Old and new meet in a soft dissolve rather than a line, widest beyond
  // the band's span where nothing else covers it. The split straightens
  // out as the ripple fades, so the far corners have turned over by the
  // time the band has gone. Before the burst it's all the live page.
  float zone = uWidth * (1.2 + 3.0 * (1.0 - spanMask));
  float mixNew = uBurst < 0.5 ? 1.0 : 1.0 - smoothstep(-zone, zone, al - (uFront - lag * uRing));
  vec4 col;
  if (mixNew > 0.999) col = soft(sp, true, blur);
  else if (mixNew < 0.001) col = soft(sp, false, blur);
  else col = mix(soft(sp, false, blur), soft(sp, true, blur), mixNew);

  // A hairline of colour either side of the crest, as soft as the content
  // around it: sharp channels over a blurred one fringe small details.
  float split = crest * uRing * spanMask * 2.0 * uScale;
  if (split > 0.3) {
    float lod = max(0.0, log2(blur * uDpr / 5.0));
    // Blended like the rest: taking these two from one page or the other drew
    // a hard green and magenta line wherever old and new differ
    col.r = mix(page(sp + dir * split, false, lod).r, page(sp + dir * split, true, lod).r, mixNew);
    col.b = mix(page(sp - dir * split, false, lod).b, page(sp - dir * split, true, lod).b, mixNew);
  }

  // While it gathers, the rest of the page dims slightly (full-page boxes
  // only: on a smaller box the dim would outline it).
  col.rgb *= 1.0 - 0.09 * uDim * uGather * (1.0 - near);

  // Premultiplied, so a box taller than its content stays clear below it
  outColor = vec4(min(col.rgb, vec3(col.a)), col.a);
}`;
var IDLE = { gather: 0, haze: 0, front: 0, width: 1, curve: 0, span: 1, center: 0.5, axis: 0, dir: [0, 1], scale: 1, dim: 0, ring: 0, burst: false, active: false };
var GlimmScope = /* @__PURE__ */ forwardRef(function GlimmScope2({
  children,
  style,
  origin = [0.5, 0],
  originOffset = [0, -12],
  direction = "ttb",
  gatherMs = 620,
  sweepMs = 1600,
  fade,
  intensity = 1,
  palette: paletteOption,
  bandTight = 28,
  peakAlpha = 1.1,
  brightness = 1.15,
  swellAmount = 0.15,
  curve = 0,
  span = 1,
  pageEffect = true
}, ref) {
  const palette = resolvePalette(paletteOption);
  const [readerOn] = usePageEffect();
  const [supported, setSupported] = useState(false);
  useEffect(() => {
    setSupported(pageEffect && readerOn && supportsHtmlInCanvas());
  }, [pageEffect, readerOn]);
  const surfaceRef = useRef(null);
  const frozen = useRef(null);
  const held = useRef(null);
  const shellRef = useRef(null);
  const pageRef = useRef(null);
  useSurface({ enabled: supported, surfaceRef, shellRef, pageRef, onFail: useCallback(() => setSupported(false), []) });
  const bandRef = useRef(null);
  const rootRef = useRef(null);
  const lifted = useRef(null);
  const ctrlRef = useRef(null);
  const raf = useRef(0);
  const current = useRef(null);
  const live = useRef({ gather: 0, strength: 0, progress: 0, origin: null });
  const config = useRef({ palette, bandTight, peakAlpha, brightness, swellAmount, curve, span, origin, originOffset, direction, gatherMs, sweepMs, fade, intensity });
  config.current = { palette, bandTight, peakAlpha, brightness, swellAmount, curve, span, origin, originOffset, direction, gatherMs, sweepMs, fade, intensity };
  useEffect(() => () => {
    cancelAnimationFrame(raf.current);
    if (current.current) current.current.done = true;
    lifted.current?.remove();
    ctrlRef.current?.destroy();
    ctrlRef.current = null;
  }, []);
  useImperativeHandle(ref, () => ({
    play(opts = {}) {
      const prev = current.current;
      if (prev && !prev.done) {
        if (document.hidden) {
          prev.abort();
          return start(opts);
        }
        if (prev.begun) {
          const pending = opts.work?.();
          if (pending) {
            pending.catch(() => {
            });
            opts = { ...opts, work: () => pending };
          }
          let landed;
          if (!pending && !opts.anticipate) {
            surfaceRef.current?.hold(true);
            if (!surfaceRef.current) held.current = freeze(frozen.current);
            const { change } = opts;
            landed = { settled: change ? flushSync(() => change(void 0)) : void 0 };
          }
          const handle = deferred();
          prev.fastForward();
          prev.dropQueued?.();
          prev.dropQueued = opts.onLeave;
          prev.next = () => handle.attach(start(opts, false, landed));
          return handle;
        }
        const keepOld = prev.committed;
        prev.abort();
        return start(opts, keepOld);
      }
      return start(opts);
    }
  }), []);
  function placeBand() {
    const holder = lifted.current, root = rootRef.current;
    if (!holder || !root) return;
    const r = root.getBoundingClientRect();
    holder.style.transform = `translate(${r.left}px, ${r.top}px)`;
    holder.style.width = r.width + "px";
    holder.style.height = r.height + "px";
  }
  function liftBand() {
    const band = bandRef.current, root = rootRef.current;
    if (lifted.current || !band || !root?.parentElement?.closest('canvas[content="drawable"]')) return;
    const holder = document.createElement("div");
    holder.style.cssText = "position:fixed;left:0;top:0;z-index:1000;pointer-events:none;overflow:hidden";
    holder.style.borderRadius = getComputedStyle(root).borderRadius;
    document.body.appendChild(holder);
    holder.appendChild(band);
    lifted.current = holder;
    placeBand();
  }
  function lowerBand() {
    const band = bandRef.current, root = rootRef.current;
    if (!lifted.current || !band || !root) return;
    root.appendChild(band);
    lifted.current.remove();
    lifted.current = null;
  }
  function deferred() {
    let real = null;
    let ff = false, cancelled = false;
    return {
      fastForward() {
        ff = true;
        real?.fastForward();
      },
      cancel() {
        cancelled = true;
        real?.cancel();
      },
      attach(h) {
        real = h;
        if (cancelled) h.cancel();
        else if (ff) h.fastForward();
      }
    };
  }
  function freeze(before = null) {
    const page = pageRef.current, shell = shellRef.current;
    if (!page || !shell) return null;
    const copy = page.cloneNode(true);
    copy.inert = true;
    copy.setAttribute("aria-hidden", "true");
    copy.setAttribute("data-glimm-frozen", "");
    Object.assign(copy.style, { position: "absolute", inset: "0", zIndex: "1", pointerEvents: "none" });
    const css = document.createElement("style");
    css.textContent = "[data-glimm-frozen] *{animation:none!important;transition:none!important}";
    copy.appendChild(css);
    shell.insertBefore(copy, before);
    const from = page.querySelectorAll("*"), to = copy.querySelectorAll("*");
    from.forEach((el, n) => {
      if (el.scrollTop || el.scrollLeft) {
        to[n].scrollTop = el.scrollTop;
        to[n].scrollLeft = el.scrollLeft;
      }
      if (el instanceof HTMLInputElement || el instanceof HTMLTextAreaElement) to[n].value = el.value;
    });
    return copy;
  }
  function start({ work, change, anticipate, at, fromOld, onBurst, onFail, onLeave }, keepOld = false, landed) {
    cancelAnimationFrame(raf.current);
    const c = config.current;
    const box = bandRef.current;
    const W = box?.clientWidth ?? 0, H = box?.clientHeight ?? 0;
    const [ox, oy] = at ?? [c.origin[0] * W + c.originOffset[0], c.origin[1] * H + c.originOffset[1]];
    const across = c.direction === "ltr" || c.direction === "rtl";
    const centerAt = (x, y) => across ? y / Math.max(1, H) : x / Math.max(1, W);
    const bandAt = (v) => across ? 1 - v : v;
    if (!ctrlRef.current && box) {
      ctrlRef.current = createShader({
        canvas: box,
        palette: c.palette,
        bandTight: c.bandTight,
        direction: c.direction,
        brightness: c.brightness,
        swellAmount: c.swellAmount,
        curve: c.curve,
        span: c.span,
        center: bandAt(centerAt(ox, oy))
      });
    }
    const ctrl = ctrlRef.current;
    ctrl?.setPalette(c.palette);
    const surface = surfaceRef.current;
    const plain = !surface;
    const run = { committed: false, begun: false, done: false, fastForward() {
    }, cancel() {
    }, abort() {
    } };
    current.current = run;
    let bursted = false;
    const burstNow = () => {
      if (!bursted) {
        bursted = true;
        onBurst?.();
      }
    };
    const settle = () => {
      ctrl?.setAlpha(0);
      surface?.set(IDLE);
      frozen.current?.remove();
      frozen.current = null;
      lowerBand();
      live.current = { gather: 0, strength: 0, progress: 0, origin: null };
    };
    const finish = () => {
      if (run.done) return;
      run.done = true;
      if (run.committed) burstNow();
      onLeave?.();
      run.next?.();
    };
    run.abort = () => {
      cancelAnimationFrame(raf.current);
      if (!run.done) {
        run.done = true;
        onLeave?.();
      }
    };
    if (!ctrl || document.hidden || matchMedia("(prefers-reduced-motion: reduce)").matches) {
      settle();
      let alive = true;
      run.cancel = () => {
        if (!run.committed) {
          alive = false;
          finish();
        }
      };
      run.abort = () => {
        alive = false;
        run.done = true;
        onLeave?.();
      };
      Promise.resolve(work?.()).then(
        (result) => {
          if (!alive || run.done) return;
          run.committed = true;
          flushSync(() => {
            change?.(result);
          });
          finish();
        },
        (error) => {
          if (alive && !run.done) {
            onFail?.(error);
            finish();
          }
        }
      );
      return run;
    }
    liftBand();
    const L = across ? W : H;
    const r = box.getBoundingClientRect();
    const fullBleed = r.width >= innerWidth - 1 && r.height >= innerHeight - 1;
    const m = fullBleed ? 0.2 : containedOverscan(c.bandTight);
    const frontAt = (p) => (-m + (1 + 2 * m) * p) * L;
    const width = 0.5 * L / Math.sqrt(c.bandTight);
    const scale = Math.min(1, Math.max(0.25, Math.min(W, H) / 600)) * c.intensity;
    const fadeAt = (p) => {
      if (!c.fade) return 1;
      const k = Math.min(1, Math.max(0, (p - c.fade[0]) / (c.fade[1] - c.fade[0])));
      return 1 - k * k * (3 - 2 * k);
    };
    const dir = across ? [c.direction === "rtl" ? -1 : 1, 0] : c.direction === "btt" ? [0, -1] : [0, 1];
    const shape = { width, curve: c.curve, span: c.span, axis: across ? 1 : 0, dir, scale, dim: fullBleed ? 1 : 0 };
    if (fromOld && !keepOld) surface?.snapshot();
    const GATHER = c.gatherMs;
    const ease = (t) => t * t * (3 - 2 * t);
    const clamp = (t) => Math.min(1, Math.max(0, t));
    const carried = live.current;
    const carries = carried.gather > 0.01 || carried.strength > 0.01;
    const originFrom = carries ? carried.origin : null;
    const t0 = performance.now();
    let readyAt = null;
    let snapFrom = 0, snapMs = 0;
    let burst = !!fromOld || keepOld;
    let letGoAt = null, letGoFrom = 0, letGoAlpha = 0;
    let runAt = null, rampFrom = 1, breakFrom = 1;
    let ffAt = null, ffFrom = 0, ffPending = false;
    const sweepMs2 = c.sweepMs;
    let hazeFrom = 0;
    let last = { gather: carries ? carried.gather : 0, haze: carries ? carried.gather : 0, progress: carries ? carried.progress : 0, strength: carries ? carried.strength : 0 };
    const paint = () => {
      const k = originFrom ? ease(clamp((performance.now() - t0) / ORIGIN_MS)) : 1;
      const x = originFrom ? originFrom[0] + (ox - originFrom[0]) * k : ox;
      const y = originFrom ? originFrom[1] + (oy - originFrom[1]) * k : oy;
      const center = centerAt(x, y);
      placeBand();
      ctrl.setCenter?.(bandAt(center));
      ctrl.setProgress(last.progress);
      ctrl.setAlpha(last.strength * c.peakAlpha);
      if (frozen.current) {
        const front = frontAt(last.progress), edge = width * 0.4;
        const side = across ? c.direction === "rtl" ? "left" : "right" : c.direction === "btt" ? "top" : "bottom";
        const mask = `linear-gradient(to ${side}, transparent ${front - edge}px, #000 ${front + edge}px)`;
        frozen.current.style.maskImage = mask;
        frozen.current.style.webkitMaskImage = mask;
      }
      surface?.setOrigin(x, y);
      surface?.set({ ...shape, center, gather: last.gather, haze: last.haze, front: frontAt(last.progress), ring: runAt !== null ? last.strength * ease(clamp((performance.now() - runAt) / RING_IN)) : 0, burst, active: true });
      live.current = { gather: last.gather, strength: last.strength, progress: last.progress, origin: [x, y] };
    };
    const begin = () => {
      run.begun = true;
      runAt = performance.now();
      rampFrom = last.strength;
      breakFrom = last.gather;
      if (ffPending) {
        ffAt = runAt;
        ffFrom = 0;
      }
    };
    const ready = () => {
      if (run.done || readyAt !== null || letGoAt !== null) return;
      if (document.hidden) {
        cancelAnimationFrame(raf.current);
        settle();
        finish();
        return;
      }
      readyAt = performance.now();
      snapFrom = last.gather;
      hazeFrom = last.haze;
      if (plain || ffPending || readyAt - t0 < INSTANT) begin();
      else snapMs = SNAP * (1 - snapFrom);
    };
    function letGo() {
      letGoAt = performance.now();
      letGoFrom = last.gather;
      letGoAlpha = last.strength;
    }
    const commit = (result) => {
      if (run.committed) return;
      run.committed = true;
      if (!fromOld && !keepOld) surface?.snapshot();
      if (plain && !fromOld) {
        if (landed) {
          frozen.current = held.current;
          held.current = null;
        } else if (!keepOld && change) frozen.current = freeze();
      }
      burst = true;
      paint();
      let settled;
      if (landed) {
        settled = landed.settled;
        surface?.hold(false);
      } else {
        try {
          settled = change ? flushSync(() => change(result)) : void 0;
        } catch (error) {
          letGo();
          throw error;
        }
      }
      if (settled && typeof settled.then === "function") {
        settled.catch(() => {
        }).then(() => requestAnimationFrame(() => requestAnimationFrame(ready)));
      } else ready();
    };
    run.cancel = () => {
      if (!run.committed && !run.done && letGoAt === null) letGo();
    };
    run.fastForward = () => {
      if (runAt === null) {
        snapMs = 0;
        ffPending = true;
        return;
      }
      if (ffAt === null) {
        ffAt = performance.now();
        ffFrom = Math.min(1, (ffAt - runAt) / sweepMs2);
      }
    };
    const frame = () => {
      const now = performance.now();
      const t = now - t0;
      let gather, haze, progress, strength;
      if (letGoAt !== null) {
        const k = clamp((now - letGoAt) / LET_GO);
        gather = letGoFrom * (1 - k) ** 2;
        haze = Math.min(last.haze, gather);
        progress = last.progress;
        strength = letGoAlpha * (1 - k) ** 2;
        if (k >= 1) {
          settle();
          finish();
          return;
        }
      } else if (runAt === null && plain) {
        gather = 0;
        haze = 0;
        progress = 0;
        strength = 0;
      } else if (runAt === null) {
        const pull = clamp(t / GATHER);
        const from = carries ? carried : { gather: 0, strength: 0, progress: 0 };
        gather = from.gather + ((work ? HOLD : 1) - from.gather) * ease(pull) * pull;
        haze = work ? from.gather * (1 - ease(pull)) : gather * (1 - ease(clamp((t - GATHER - HOLD_AFTER) / RELAX)));
        strength = from.strength + ((work ? 0.55 : 1) - from.strength) * ease(pull);
        progress = from.progress + (PEEK - from.progress) * ease(pull);
        if (readyAt !== null) {
          const k = snapMs > 0 ? clamp((now - readyAt) / snapMs) : 1;
          gather = snapFrom + (1 - snapFrom) * ease(k);
          haze = hazeFrom + (1 - hazeFrom) * ease(k);
          strength = Math.max(strength, ease(k));
          last = { gather, haze, progress, strength };
          if (k >= 1) begin();
        }
      } else {
        const since = now - runAt;
        if (since >= BURST_AT) burstNow();
        gather = breakFrom * Math.max(0, 1 - since / RELEASE) ** 2;
        haze = gather;
        let k = clamp(since / sweepMs2);
        if (ffAt !== null) k = ffFrom + (1 - ffFrom) * ease(clamp((now - ffAt) / FAST));
        progress = PEEK + (1 - PEEK) * (1 - (1 - k) ** 2.4);
        const ramp = rampFrom + (1 - rampFrom) * ease(clamp(since / RAMP));
        strength = (1 - k) ** 1.2 * fadeAt(progress) * ramp;
        if (k >= 1 || c.fade && progress >= c.fade[1]) {
          settle();
          finish();
          return;
        }
      }
      last = { gather, haze, progress, strength };
      paint();
      raf.current = requestAnimationFrame(frame);
    };
    raf.current = requestAnimationFrame(frame);
    const job = landed ? null : work ?? (anticipate && !plain ? () => new Promise((done) => setTimeout(done, GATHER)) : null);
    if (job) {
      new Promise((done) => done(job())).then(
        (value) => {
          if (!run.done && letGoAt === null) commit(value);
        },
        (error) => {
          if (run.done || letGoAt !== null) return;
          onFail?.(error);
          if (keepOld) ready();
          else letGo();
        }
      );
    } else commit(void 0);
    return run;
  }
  return /* @__PURE__ */ jsxs("div", { ref: rootRef, style: { position: "relative", overflow: "hidden", isolation: "isolate", ...style }, children: [
    /* @__PURE__ */ jsx("div", { ref: shellRef, style: { position: "relative", width: "100%", height: "100%" }, children: /* @__PURE__ */ jsx("div", { ref: pageRef, style: { position: "relative", width: "100%", height: "100%", overflow: "hidden" }, children }) }),
    /* @__PURE__ */ jsx(
      "canvas",
      {
        ref: bandRef,
        "aria-hidden": true,
        style: { position: "absolute", inset: 0, width: "100%", height: "100%", display: "block", pointerEvents: "none" }
      }
    )
  ] });
});
function useSurface({ enabled, surfaceRef, shellRef, pageRef, onFail }) {
  useEffect(() => {
    if (!enabled) return;
    const shell = shellRef.current;
    const el = pageRef.current;
    const canvas = document.createElement("canvas");
    canvas.setAttribute("content", "drawable");
    canvas.style.cssText = "position:absolute;inset:0;width:100%;height:100%;display:block";
    const gl = canvas.getContext("webgl2", { premultipliedAlpha: true, antialias: false });
    if (!gl) {
      onFail();
      return;
    }
    el.setAttribute("drawable", "");
    shell.appendChild(canvas);
    canvas.appendChild(el);
    const g = gl;
    const c = canvas;
    const compile = (type, src) => {
      const s = gl.createShader(type);
      gl.shaderSource(s, src);
      gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) console.error("[glimm]", gl.getShaderInfoLog(s));
      return s;
    };
    const prog = gl.createProgram();
    gl.attachShader(prog, compile(gl.VERTEX_SHADER, VS2));
    gl.attachShader(prog, compile(gl.FRAGMENT_SHADER, FS2));
    gl.linkProgram(prog);
    gl.useProgram(prog);
    const buf = gl.createBuffer();
    gl.bindBuffer(gl.ARRAY_BUFFER, buf);
    gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1, -1, 1, -1, -1, 1, 1, 1]), gl.STATIC_DRAW);
    const aPos = gl.getAttribLocation(prog, "aPos");
    gl.enableVertexAttribArray(aPos);
    gl.vertexAttribPointer(aPos, 2, gl.FLOAT, false, 0, 0);
    const U = (n) => gl.getUniformLocation(prog, n);
    const uRes = U("uRes"), uDpr = U("uDpr"), uO = U("uO"), uGather = U("uGather"), uHaze = U("uHaze"), uFront = U("uFront"), uCurve = U("uCurve"), uSpan = U("uSpan"), uCenter = U("uCenter"), uAxis = U("uAxis"), uDir = U("uDir"), uScale = U("uScale"), uDim = U("uDim"), uWidth = U("uWidth"), uRing = U("uRing"), uBurst = U("uBurst"), uActive = U("uActive");
    gl.uniform1i(U("uNew"), 0);
    gl.uniform1i(U("uOld"), 1);
    const makeTex = () => {
      const t = gl.createTexture();
      gl.bindTexture(gl.TEXTURE_2D, t);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      return t;
    };
    const texNew = makeTex();
    const texOld = makeTex();
    const fbo = gl.createFramebuffer();
    let tw = 0, th = 0, pad = 0;
    let state = IDLE;
    let origin = [0, 0];
    let mipsStale = true;
    let oldValid = false;
    let held = false;
    const buildMips = () => {
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, texNew);
      gl.generateMipmap(gl.TEXTURE_2D);
      mipsStale = false;
    };
    const draw = () => {
      const W = canvas.clientWidth, H = canvas.clientHeight;
      gl.viewport(0, 0, canvas.width, canvas.height);
      gl.uniform2f(uRes, W, H);
      gl.uniform1f(uDpr, canvas.width / Math.max(1, W));
      gl.uniform2f(uO, origin[0], origin[1]);
      gl.uniform1f(uGather, state.gather);
      gl.uniform1f(uHaze, state.haze);
      gl.uniform1f(uFront, state.front);
      gl.uniform1f(uCurve, state.curve);
      gl.uniform1f(uSpan, state.span);
      gl.uniform1f(uCenter, state.center);
      gl.uniform1f(uAxis, state.axis);
      gl.uniform2f(uDir, state.dir[0], state.dir[1]);
      gl.uniform1f(uScale, state.scale);
      gl.uniform1f(uDim, state.dim);
      gl.uniform1f(uWidth, state.width);
      gl.uniform1f(uRing, state.ring);
      gl.uniform1f(uBurst, state.burst && oldValid ? 1 : 0);
      gl.uniform1f(uActive, state.active ? 1 : 0);
      gl.activeTexture(gl.TEXTURE1);
      gl.bindTexture(gl.TEXTURE_2D, texOld);
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, texNew);
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    };
    const resize = () => {
      const dpr = Math.min(window.devicePixelRatio || 1, 2);
      canvas.width = Math.round(canvas.clientWidth * dpr);
      canvas.height = Math.round(canvas.clientHeight * dpr);
      c.requestPaint?.();
    };
    let painted = false;
    const noPaint = window.setTimeout(() => {
      if (!painted && !document.hidden) onFail();
    }, 1e3);
    const onLost = () => onFail();
    canvas.addEventListener("webglcontextlost", onLost);
    const onPaint = () => {
      painted = true;
      if (held) {
        c.updateElementGeometry?.(el, { canvasTransform: new DOMMatrix() });
        return;
      }
      const size = c.getElementImageDefaultSize?.(el) ?? { width: canvas.width, height: canvas.height };
      const alloc = (w, h) => {
        if (w === tw && h === th) return;
        tw = w;
        th = h;
        oldValid = false;
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
        gl.bindTexture(gl.TEXTURE_2D, texOld);
        gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, w, h, 0, gl.RGBA, gl.UNSIGNED_BYTE, null);
        gl.bindTexture(gl.TEXTURE_2D, texNew);
      };
      gl.activeTexture(gl.TEXTURE0);
      gl.bindTexture(gl.TEXTURE_2D, texNew);
      const fresh = tw === 0;
      alloc(Math.ceil(size.width) + pad, Math.ceil(size.height) + pad);
      try {
        while (gl.getError()) {
        }
        g.texElementSubImage2D(gl.TEXTURE_2D, 0, 0, 0, el);
        if (gl.getError() === gl.INVALID_VALUE && pad < 2) {
          pad++;
          alloc(tw + 1, th + 1);
          g.texElementSubImage2D(gl.TEXTURE_2D, 0, 0, 0, el);
        }
        mipsStale = true;
        if (state.active || fresh) buildMips();
      } catch (error) {
        console.warn("[glimm] upload failed", error);
      }
      c.updateElementGeometry?.(el, { canvasTransform: new DOMMatrix() });
      draw();
    };
    canvas.addEventListener("paint", onPaint);
    const ro = new ResizeObserver(resize);
    ro.observe(canvas);
    resize();
    surfaceRef.current = {
      snapshot() {
        if (!tw) return;
        gl.bindFramebuffer(gl.FRAMEBUFFER, fbo);
        gl.framebufferTexture2D(gl.FRAMEBUFFER, gl.COLOR_ATTACHMENT0, gl.TEXTURE_2D, texNew, 0);
        gl.activeTexture(gl.TEXTURE1);
        gl.bindTexture(gl.TEXTURE_2D, texOld);
        gl.copyTexSubImage2D(gl.TEXTURE_2D, 0, 0, 0, 0, 0, tw, th);
        gl.generateMipmap(gl.TEXTURE_2D);
        gl.bindFramebuffer(gl.FRAMEBUFFER, null);
        oldValid = true;
      },
      set(next) {
        state = next;
        if (next.active && mipsStale) buildMips();
        draw();
      },
      setOrigin(x, y) {
        origin = [x, y];
      },
      hold(on) {
        held = on;
        if (!on) c.requestPaint?.();
      }
    };
    return () => {
      surfaceRef.current = null;
      clearTimeout(noPaint);
      canvas.removeEventListener("webglcontextlost", onLost);
      canvas.removeEventListener("paint", onPaint);
      ro.disconnect();
      gl.deleteProgram(prog);
      gl.deleteBuffer(buf);
      gl.deleteTexture(texNew);
      gl.deleteTexture(texOld);
      gl.deleteFramebuffer(fbo);
      el.removeAttribute("drawable");
      shell.appendChild(el);
      canvas.remove();
    };
  }, [enabled, surfaceRef, shellRef, pageRef, onFail]);
}

export { EASINGS, GlimmProvider, GlimmScope, PALETTES, getPageEffect, setPageEffect, supportsHtmlInCanvas, useGlimm, usePageEffect };
