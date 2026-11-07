import fs from 'node:fs';
import path from 'node:path';
import { createRequire } from 'node:module';

const require = createRequire(import.meta.url);
let playwright;
for (const candidate of ['playwright-core', ...process.env.PATH.split(path.delimiter).filter(p => p.endsWith('node_modules/.bin')).map(p => path.join(path.dirname(p), 'playwright-core'))]) {
  try { playwright = require(candidate); break; } catch {}
}
if (!playwright) throw new Error('Run with: npm exec --yes --package=playwright-core -- node verify-glimm.mjs');
const core = fs.readFileSync('glimm-reference/package/dist/index.js', 'utf8');
const reactFile = fs.readdirSync('glimm-reference/package/dist').find(f => f.startsWith('chunk-'));
const react = fs.readFileSync(path.join('glimm-reference/package/dist', reactFile), 'utf8');
const extract = (text, name) => {
  const match = text.match(new RegExp('var ' + name + ' = `([\\s\\S]*?)`;'));
  if (!match) throw new Error(`Missing published shader ${name}`);
  return match[1];
};
const uniforms = JSON.parse(fs.readFileSync('glimm-uniforms.json', 'utf8'));
const png = file => 'data:image/png;base64,' + fs.readFileSync(file).toString('base64');
const browser = await playwright.chromium.launch({ executablePath: process.env.CHROME_PATH || '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true });
try {
  const page = await browser.newPage();
  await page.setContent('<html><body></body></html>');
  const result = await page.evaluate(async ({ pageVS, pageFS, bandFS, u, livePNG, oldPNG, metalPNG }) => {
    const width = 512, height = 320;
    const canvas = document.createElement('canvas'); canvas.width = width; canvas.height = height;
    const gl = canvas.getContext('webgl2', { alpha: true, premultipliedAlpha: true, antialias: false });
    if (!gl) throw new Error('WebGL2 unavailable');
    const shader = (type, source) => {
      const s = gl.createShader(type); gl.shaderSource(s, source); gl.compileShader(s);
      if (!gl.getShaderParameter(s, gl.COMPILE_STATUS)) throw new Error(gl.getShaderInfoLog(s));
      return s;
    };
    const program = (vs, fs) => {
      const p = gl.createProgram(); gl.attachShader(p, shader(gl.VERTEX_SHADER, vs)); gl.attachShader(p, shader(gl.FRAGMENT_SHADER, fs)); gl.linkProgram(p);
      if (!gl.getProgramParameter(p, gl.LINK_STATUS)) throw new Error(gl.getProgramInfoLog(p));
      return p;
    };
    const pixels = async src => {
      const image = new Image(); image.src = src; await image.decode();
      const c = document.createElement('canvas'); c.width = width; c.height = height;
      const ctx = c.getContext('2d'); ctx.drawImage(image, 0, 0);
      return ctx.getImageData(0, 0, width, height).data;
    };
    const texture = async (src, unit) => {
      gl.activeTexture(gl.TEXTURE0 + unit);
      const t = gl.createTexture(); gl.bindTexture(gl.TEXTURE_2D, t);
      gl.texImage2D(gl.TEXTURE_2D, 0, gl.RGBA8, width, height, 0, gl.RGBA, gl.UNSIGNED_BYTE, await pixels(src));
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MIN_FILTER, gl.LINEAR_MIPMAP_LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_MAG_FILTER, gl.LINEAR);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_S, gl.CLAMP_TO_EDGE);
      gl.texParameteri(gl.TEXTURE_2D, gl.TEXTURE_WRAP_T, gl.CLAMP_TO_EDGE);
      gl.generateMipmap(gl.TEXTURE_2D);
      return t;
    };
    await texture(livePNG, 0); await texture(oldPNG, 1);
    const draw = (p, values) => {
      gl.useProgram(p);
      const buffer = gl.createBuffer(); gl.bindBuffer(gl.ARRAY_BUFFER, buffer);
      gl.bufferData(gl.ARRAY_BUFFER, new Float32Array([-1,-1,1,-1,-1,1,1,1]), gl.STATIC_DRAW);
      const loc = gl.getAttribLocation(p, 'aPos'); gl.enableVertexAttribArray(loc); gl.vertexAttribPointer(loc, 2, gl.FLOAT, false, 0, 0);
      for (const [name, value] of Object.entries(values)) {
        const location = gl.getUniformLocation(p, name);
        if (name === 'uNew' || name === 'uOld') gl.uniform1i(location, value);
        else if (Array.isArray(value)) gl[`uniform${value.length}fv`](location, value);
        else gl.uniform1f(location, value);
      }
      gl.drawArrays(gl.TRIANGLE_STRIP, 0, 4);
    };
    const dir = u[2].slice(2), horizontal = Math.abs(dir[0]) > 0.5;
    gl.viewport(0, 0, width, height);
    draw(program(pageVS, pageFS), {
      uNew: 0, uOld: 1, uRes: [width,height], uDpr: u[0][2], uO: u[2].slice(0,2), uDir: dir, uAxis: +horizontal,
      uScale: u[4][1], uDim: u[4][2], uGather: u[3][2], uHaze: u[3][3], uFront: u[8][2], uWidth: u[8][3], uRing: u[4][0],
      uCurve: u[5][1], uSpan: u[5][2], uCenter: u[5][3], uBurst: u[4][3], uActive: 1,
    });
    gl.enable(gl.BLEND); gl.blendFunc(gl.ONE, gl.ONE_MINUS_SRC_ALPHA);
    const modernBand = '#version 300 es\n' + bandFS.replace('void main()', 'out vec4 outColor;\nvoid main()').replaceAll('gl_FragColor', 'outColor');
    draw(program(pageVS, modernBand), {
      uRes: [width,height], uTime: u[0][3], uProgress: u[3][0], uAlpha: u[3][1], uBandTight: u[5][0],
      uPalA: u[9].slice(0,3), uPalB: u[10].slice(0,3), uPalC: u[11].slice(0,3), uPalD: u[12].slice(0,3), uHueShift: u[7][1],
      uPosStart: u[8][0], uPosEnd: u[8][1], uDirection: +!horizontal, uWaveAmount: u[6][0], uRippleAmount: u[6][1], uWaveSpeed: u[6][2],
      uBrightness: u[6][3], uSwellAmount: u[7][0], uCurve: u[5][1], uSpan: u[5][2], uCenter: horizontal ? 1-u[5][3] : u[5][3], uFeather: u[7][2],
    });
    if (gl.getError()) throw new Error('WebGL error');
    const raw = new Uint8Array(width*height*4); gl.readPixels(0,0,width,height,gl.RGBA,gl.UNSIGNED_BYTE,raw);
    const flipped = new Uint8ClampedArray(raw.length);
    for (let y=0; y<height; y++) flipped.set(raw.subarray(y*width*4,(y+1)*width*4),(height-y-1)*width*4);
    const actual = await pixels(metalPNG), diff = new Uint8ClampedArray(raw.length);
    let sum = 0, max = 0, over3 = 0;
    for (let i=0; i<actual.length; i+=4) {
      for (let c=0; c<3; c++) {
        const d = Math.abs(actual[i+c]-flipped[i+c]); sum += d; max = Math.max(max,d); if (d>3) over3++;
        diff[i+c] = Math.min(255,d*12);
      }
      diff[i+3] = 255;
    }
    const encode = values => {
      const c = document.createElement('canvas'); c.width = width; c.height = height;
      c.getContext('2d').putImageData(new ImageData(values,width,height),0,0); return c.toDataURL('image/png').split(',')[1];
    };
    return { meanAbsoluteError: sum/(width*height*3), maxChannelError: max, channelsOver3: over3, channelCount: width*height*3, reference: encode(flipped), diff: encode(diff) };
  }, { pageVS: extract(react, 'VS2'), pageFS: extract(react, 'FS2'), bandFS: extract(core, 'FS'), u: uniforms, livePNG: png('glimm-live.png'), oldPNG: png('glimm-old.png'), metalPNG: png('glimm-metal.png') });
  fs.writeFileSync('glimm-webgl-reference.png', Buffer.from(result.reference, 'base64'));
  fs.writeFileSync('glimm-parity-diff.png', Buffer.from(result.diff, 'base64'));
  delete result.reference; delete result.diff;
  fs.writeFileSync('glimm-parity-results.json', JSON.stringify(result,null,2)+'\n');
  console.log(result);
  if (result.meanAbsoluteError > 1 || result.maxChannelError > 8) process.exitCode = 1;
} finally { await browser.close(); }
