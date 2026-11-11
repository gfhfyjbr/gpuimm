import fs from 'node:fs/promises';
import path from 'node:path';
import crypto from 'node:crypto';
import { createRequire } from 'node:module';
const require = createRequire(import.meta.url);
const { chromium } = require('/Users/gfhfyjbr/.npm/_npx/e058441c325e062a/node_modules/playwright-core');
const out = path.resolve('crates/gpui_perf/src/fast/glimm_assets');
await fs.mkdir(out, { recursive: true });
const browser = await chromium.launch({ executablePath: '/Applications/Google Chrome.app/Contents/MacOS/Google Chrome', headless: true, args: ['--enable-experimental-web-platform-features'] });
const context = await browser.newContext({ viewport: { width: 634, height: 476 }, deviceScaleFactor: 2 });
await context.addInitScript(()=> {
  const bounds=Element.prototype.getBoundingClientRect;
  Element.prototype.getBoundingClientRect=function(){
    const b=bounds.call(this);
    if(location.pathname.endsWith('/reset')&&new Error().stack.includes('138--eewq9_hk')) {
      (window.__resetRectCalls??=[]).push({tag:this.tagName,className:this.className,bounds:[b.x,b.y,b.width,b.height]});
      if(typeof this.className==='string'&&this.className.includes('composerWidth'))window.__resetMeasured=[b.x,b.y,b.width,b.height];
    }
    return b;
  };
  for(const prototype of [WebGLRenderingContext.prototype,WebGL2RenderingContext.prototype]) {
    const names=new WeakMap(),get=prototype.getUniformLocation,set=prototype.uniform2f,set1=prototype.uniform1f;
    prototype.getUniformLocation=function(program,name){const location=get.call(this,program,name);if(location)names.set(location,name);return location;};
    prototype.uniform2f=function(location,x,y){if(names.get(location)==='uO')window.__glimmEffectOrigin=[x,y];return set.call(this,location,x,y);};
    prototype.uniform1f=function(location,value){const name=names.get(location);if(name)(window.__glimmUniforms??={})[name]=value;return set1.call(this,location,value);};
  }
});
const decorations = new Map();
const scenes = {};
async function snapshot(frame, name) {
  const scene = await frame.evaluate(() => {
    const nodes = [], controls = [], anchors = {};
    const rect = e => { const r = e.getBoundingClientRect(); return [r.x, r.y, r.width, r.height]; };
    const intersect = (a,b) => { const x=Math.max(a[0],b[0]),y=Math.max(a[1],b[1]);return [x,y,Math.max(0,Math.min(a[0]+a[2],b[0]+b[2])-x),Math.max(0,Math.min(a[1]+a[3],b[1]+b[3])-y)]; };
    const scope = document.querySelector('canvas[content="drawable"]');
    const scopeBounds = scope ? rect(scope) : [0,0,innerWidth,innerHeight];
    let next = 0;
    function textNodes(node, style, clip, opacity, inside, id) {
      if (!node.textContent.trim() || clip[2]<=0 || clip[3]<=0) return;
      const lines = [];
      for(let i=0;i<node.textContent.length;i++) {
        const r = document.createRange();r.setStart(node,i);r.setEnd(node,i+1);
        const b = r.getBoundingClientRect();
        if (!b.width || !b.height) continue;
        let line=lines.find(l=>Math.abs(l.bounds[1]-b.y)<0.5);
        if(!line){line={text:'',bounds:[b.x,b.y,b.width,b.height]};lines.push(line);}
        line.text+=node.textContent[i];line.bounds[2]=b.right-line.bounds[0];
      }
      for(const [i,line] of lines.entries()) nodes.push({id:`${id}:text${i}`,kind:'text',...line,clip,opacity,inside,color:style.color,fontSize:parseFloat(style.fontSize),weight:parseFloat(style.fontWeight),font:style.fontFamily,tracking:parseFloat(style.letterSpacing)||0});
    }
    function visit(e, clip=[0,0,innerWidth,innerHeight], opacity=1, inside=false, rounded=[]) {
      if (!(e instanceof Element) || ['SCRIPT','STYLE','LINK','NOSCRIPT','NEXT-ROUTE-ANNOUNCER'].includes(e.tagName)) return;
      const s=getComputedStyle(e), b=rect(e), id=`n${next++}`;
      opacity*=Number(s.opacity);
      if(s.display==='none'||s.visibility==='hidden'||opacity<0.001||s.clip!=='auto'||s.clipPath==='inset(50%)'||e.className?.toString().includes('sr-only'))return;
      if(e===scope) inside=true;
      if(e.tagName==='CANVAS'&&e!==scope)return;
      // The drawable canvas contains the live native-equivalent DOM, not its WebGL pixels.
      const clipped=intersect(clip,b);
      const visible=clipped[2]>0&&clipped[3]>0;
      const radii=[s.borderTopLeftRadius,s.borderTopRightRadius,s.borderBottomRightRadius,s.borderBottomLeftRadius].map(v=>v.includes('%')?Math.min(b[2],b[3])*parseFloat(v)/100:Math.min(parseFloat(v)||0,Math.min(b[2],b[3])/2));
      for(const parent of rounded) {
        const p=parent.bounds,r=parent.radii;
        if(Math.abs(b[0]-p[0])<=2&&Math.abs(b[1]-p[1])<=2)radii[0]=Math.max(radii[0],r[0]-Math.max(b[0]-p[0],b[1]-p[1]));
        if(Math.abs(b[0]+b[2]-p[0]-p[2])<=2&&Math.abs(b[1]-p[1])<=2)radii[1]=Math.max(radii[1],r[1]-Math.max(p[0]+p[2]-b[0]-b[2],b[1]-p[1]));
        if(Math.abs(b[0]+b[2]-p[0]-p[2])<=2&&Math.abs(b[1]+b[3]-p[1]-p[3])<=2)radii[2]=Math.max(radii[2],r[2]-Math.max(p[0]+p[2]-b[0]-b[2],p[1]+p[3]-b[1]-b[3]));
        if(Math.abs(b[0]-p[0])<=2&&Math.abs(b[1]+b[3]-p[1]-p[3])<=2)radii[3]=Math.max(radii[3],r[3]-Math.max(b[0]-p[0],p[1]+p[3]-b[1]-b[3]));
      }
      const border=[s.borderTopWidth,s.borderRightWidth,s.borderBottomWidth,s.borderLeftWidth].map(v=>parseFloat(v)||0);
      if(visible&&location.pathname.endsWith('/invoice')&&s.width==='46px'&&s.height==='40px') {
        const clone=e.cloneNode(true),src=[e,...e.querySelectorAll('*')],dst=[clone,...clone.querySelectorAll('*')];
        src.forEach((el,i)=>{const style=getComputedStyle(el);for(const key of style)dst[i].style.setProperty(key,style.getPropertyValue(key));});
        Object.assign(clone.style,{position:'relative',left:'0',top:'0',margin:'0',opacity:'1'});
        nodes.push({id:id+':illustration',kind:'image',bounds:b,clip,opacity,inside,decoration:{type:'html',width:b[2],height:b[3],html:clone.outerHTML}});
        return;
      }
      if(visible&&s.backdropFilter!=='none')nodes.push({id:id+':backdrop',kind:'image',bounds:b,clip,opacity,inside,decoration:{type:'backdrop',width:b[2],height:b[3],bounds:b,radii,filter:s.backdropFilter,under:structuredClone(nodes)}});
      if(visible && (s.backgroundColor!=='rgba(0, 0, 0, 0)' || border.some(v=>v>0) || s.boxShadow!=='none')) {
        nodes.push({id,kind:'box',bounds:b,clip,opacity,inside,color:s.backgroundColor,radii,border,borderColor:[s.borderTopColor,s.borderRightColor,s.borderBottomColor,s.borderLeftColor][Math.max(0,border.findIndex(v=>v>0))],shadow:s.boxShadow});
      }
      if(visible && s.outlineStyle!=='none' && parseFloat(s.outlineWidth)>0) {
        const w=parseFloat(s.outlineWidth),o=parseFloat(s.outlineOffset)||0;
        nodes.push({id:id+':outline',kind:'box',bounds:[b[0]-o-w,b[1]-o-w,b[2]+2*(o+w),b[3]+2*(o+w)],clip,opacity,inside,color:'transparent',radii:[0,0,0,0],border:[w,w,w,w],borderColor:s.outlineColor,shadow:'none'});
      }
      if(visible&&s.backgroundImage!=='none') nodes.push({id:id+':bg',kind:'image',bounds:b,clip,opacity,inside,decoration:{type:'css',width:b[2],height:b[3],cssWidth:parseFloat(s.width)||b[2],cssHeight:parseFloat(s.height)||b[3],style:{backgroundImage:s.backgroundImage,backgroundSize:s.backgroundSize,backgroundPosition:s.backgroundPosition,backgroundRepeat:s.backgroundRepeat,borderRadius:s.borderRadius,cornerShape:s.cornerShape,clipPath:s.clipPath,maskImage:s.maskImage,maskSize:s.maskSize,maskPosition:s.maskPosition}}});
      if(visible&&e.tagName==='svg') {
        const clone=e.cloneNode(true), src=[e,...e.querySelectorAll('*')], dst=[clone,...clone.querySelectorAll('*')];
        src.forEach((el,i)=>{const cs=getComputedStyle(el);for(const k of ['fill','stroke','stroke-width','stroke-dasharray','stroke-linecap','stroke-linejoin','opacity'])dst[i].style.setProperty(k,cs.getPropertyValue(k));});
        clone.setAttribute('xmlns','http://www.w3.org/2000/svg');clone.style.transform='none';clone.style.width='100%';clone.style.height='100%';
        nodes.push({id:id+':svg',kind:'image',bounds:b,clip,opacity,inside,decoration:{type:'svg',width:b[2],height:b[3],html:clone.outerHTML}});
        return;
      }
      if(visible&&e.tagName==='IMG') {
        let p=e.parentElement;
        while(p.parentElement && !(getComputedStyle(p).overflow==='hidden'&&parseFloat(getComputedStyle(p).borderTopLeftRadius)>0))p=p.parentElement;
        if(p===document.body||p===document.documentElement)p=e;
        const pb=rect(p),ps=getComputedStyle(p);
        const radius=ps.borderTopLeftRadius.includes('%')?Math.min(pb[2],pb[3])*parseFloat(ps.borderTopLeftRadius)/100:parseFloat(ps.borderTopLeftRadius)||0;
        nodes.push({id:id+':photo',kind:'image',bounds:pb,clip,opacity,inside,decoration:{type:'photo',width:pb[2],height:pb[3],src:e.currentSrc,imgBounds:[b[0]-pb[0],b[1]-pb[1],b[2],b[3]],radius,objectFit:s.objectFit}});
        return;
      }
      if(visible && ['BUTTON','TEXTAREA','INPUT'].includes(e.tagName) && !e.closest('[inert]')) {
        const label=e.getAttribute('aria-label')||e.innerText||'';
        controls.push({id,label,bounds:b,disabled:e.disabled||false,role:e.getAttribute('role')||e.tagName.toLowerCase(),checked:e.getAttribute('aria-checked')==='true'});
        if(label==='Prompt'||label==='Message') {
          const composer=e.parentElement;
          anchors.composer=rect(composer);
          if(label==='Message')anchors.resetDock=rect(composer.parentElement);
        }
      }
      if(visible && e.tagName==='TEXTAREA' && e.value) {
        const mirror=document.createElement('div');
        for(const k of ['font','fontFamily','fontSize','fontWeight','lineHeight','letterSpacing','padding','textWrap','whiteSpace','boxSizing'])mirror.style[k]=s[k];
        Object.assign(mirror.style,{position:'fixed',left:b[0]+'px',top:b[1]+'px',width:b[2]+'px',whiteSpace:'pre-wrap'});mirror.textContent=e.value;document.body.append(mirror);
        textNodes(mirror.firstChild,s,clip,opacity,inside,id);mirror.remove();
      }
      let childClip=clip;
      if(e!==document.body && e!==document.documentElement && (['hidden','clip','auto','scroll'].includes(s.overflowX)||['hidden','clip','auto','scroll'].includes(s.overflowY)))childClip=intersect(clip,b);
      // CSS paints positioned positive-z children after the normal flow.
      const children=[...e.childNodes].map((n,i)=>({n,i,z:n instanceof Element?(parseInt(getComputedStyle(n).zIndex)||0):0})).sort((a,b)=>a.z-b.z||a.i-b.i);
      const childRounded=radii.some(r=>r>0)&&childClip!==clip?[...rounded,{bounds:b,radii}]:rounded;
      for(const {n} of children){if(n.nodeType===Node.TEXT_NODE)textNodes(n,s,childClip,opacity,inside,id);else visit(n,childClip,opacity,inside,childRounded);}
    }
    visit(document.querySelector('.theme-transit')?.closest('body') || document.body);
    const tom=[...document.querySelectorAll('img[alt="Tom Becker"]')].find(e=>rect(e)[1]<52);
    if(tom)anchors.joinAvatar=rect(tom);
    if(window.__resetMeasured)anchors.resetMeasured=window.__resetMeasured;
    return {width:innerWidth,height:innerHeight,scope:scopeBounds,nodes,controls,anchors,effectOrigin:window.__glimmEffectOrigin,uniforms:window.__glimmUniforms,resetRectCalls:window.__resetRectCalls};
  });
  for (const node of scene.nodes) if(node.decoration) {
    const key=JSON.stringify(node.decoration), hash=crypto.createHash('sha256').update(key).digest('hex').slice(0,16);
    node.asset=hash+'.png';decorations.set(node.asset,node.decoration);delete node.decoration;
  }
  scenes[name]=scene;
  console.log(name,scene.nodes.length,'native primitives',scene.controls.map(c=>c.label));
}
async function study(route) {
  const page=await context.newPage();
  await page.route('https://glimm.dev/native-reference',r=>r.fulfill({contentType:'text/html',body:`<!doctype html><style>html,body{margin:0;width:634px;height:476px;overflow:hidden}iframe{width:634px;height:476px;border:0}</style><iframe src="/demos/${route}"></iframe>`}));
  await page.goto('https://glimm.dev/native-reference');
  const frame=await(await page.locator('iframe').elementHandle()).contentFrame();
  await frame.waitForFunction(()=>document.fonts.status==='loaded'&&!!document.querySelector('canvas[content="drawable"]'));
  await frame.addStyleTag({content:`html,body,[style*="overflow"]:not(textarea),[class*="overflow-"]:not([class*="overflow-visible"]){overflow:hidden!important}nav[aria-label="Switch demo"],nextjs-portal,.glimm-hint-in{display:none!important}:root{--cta-bottom:16px;--composer-max:none;--composer-corner:12px;--composer-curve:round}`});
  await page.waitForTimeout(80);
  await snapshot(frame,route+'-initial');
  await page.screenshot({path:`glimm-original-${route}-before.png`});
  if(route==='sheet') {
    await frame.getByRole('button',{name:'Generate',exact:true}).click();await page.waitForTimeout(450);await snapshot(frame,'sheet-busy');
    await page.waitForTimeout(5700);await snapshot(frame,'sheet-final');
  } else if(route==='upgrade') {
    await frame.getByRole('button',{name:'Upgrade',exact:true}).click();await page.waitForTimeout(150);await snapshot(frame,'upgrade-busy');
    await page.waitForTimeout(4000);await snapshot(frame,'upgrade-final');
  } else if(route==='publish') {
    await frame.getByRole('button',{name:'Publish',exact:true}).click();await page.waitForTimeout(150);await snapshot(frame,'publish-busy');
    await page.waitForTimeout(5700);await frame.waitForFunction(()=>document.fonts.status==='loaded');await snapshot(frame,'publish-final');
  } else if(route==='autopilot'||route==='theme') {
    await frame.getByRole('switch',{name:route==='theme'?'Theme':'Autopilot'}).click();await page.waitForTimeout(150);await snapshot(frame,route+'-busy');
    await page.waitForTimeout(4200);await snapshot(frame,route+'-final');
  } else if(route==='tasks') {
    for(let mask=0;mask<32;mask++) {
      for(let i=0;i<5;i++) {
        const box=frame.getByRole('checkbox',{name:`Task ${i+1}`,exact:true});
        if((await box.getAttribute('aria-checked')==='true')!==!!(mask&(1<<i)))await box.click();
      }
      if(mask===31) {await snapshot(frame,'tasks-complete');await page.waitForTimeout(1150);} else await page.waitForTimeout(190);
      await snapshot(frame,`tasks-${mask}`);
    }
    await page.waitForTimeout(2350);await snapshot(frame,'tasks-31');await snapshot(frame,'tasks-final');
  } else if(route==='reset') {
    await page.waitForTimeout(3000);await snapshot(frame,'reset-busy');await page.waitForTimeout(3000);await snapshot(frame,'reset-final');
  } else if(route==='join') {
    await page.waitForTimeout(1400);await snapshot(frame,'join-arriving');
    await page.waitForTimeout(2800);await snapshot(frame,'join-final');
  } else {
    await page.waitForTimeout(6000);await snapshot(frame,route+'-final');
  }
  await page.screenshot({path:`glimm-original-${route}-after.png`});
  await page.close();
}
try {
  await Promise.all(['sheet','join','invoice','upgrade','reset','publish','autopilot','theme','tasks'].map(study));
  const joined=scenes['join-final'];const popover=joined.nodes.findIndex(n=>!n.inside&&n.bounds[1]>=52&&n.shadow&&n.shadow!=='none');
  if(popover<0)throw new Error('Original teammate popover missing');
  scenes['join-arriving']={...structuredClone(joined),nodes:structuredClone(joined.nodes.slice(0,popover)),controls:joined.controls.filter(c=>c.label==='Replay the join')};
  const upgrade=scenes['upgrade-initial'];let cutoff=upgrade.nodes.findIndex(n=>n.shadow?.includes('48px 96px'));
  if(cutoff<0)throw new Error('Original upgrade card missing');
  if(upgrade.nodes[cutoff-1]?.id.endsWith(':backdrop'))cutoff--;
  const scrim=upgrade.nodes.findIndex(n=>n.color?.replace(/\s/g,'')==='rgba(24,24,27,0.14)');
  if(scrim>=0)cutoff=Math.min(cutoff,scrim);
  scenes['upgrade-closed']={...structuredClone(upgrade),nodes:structuredClone(upgrade.nodes.slice(0,cutoff)),controls:[]};
  await fs.writeFile(path.join(out,'scenes.json'),JSON.stringify(scenes));
  const graphicsPage=await context.newPage();
  for(const [name,d] of decorations) {
    const file=path.join(out,name);
    let photo;
    if(d.type==='photo') {const response=await fetch(d.src);photo=`data:${response.headers.get('content-type')};base64,${Buffer.from(await response.arrayBuffer()).toString('base64')}`;}
    const encoded=await graphicsPage.evaluate(async({d,photo})=> {
      async function raster(d) {
        const canvas=document.createElement('canvas');canvas.width=Math.max(1,Math.ceil(d.width*2));canvas.height=Math.max(1,Math.ceil(d.height*2));
        const ctx=canvas.getContext('2d');ctx.scale(2,2);
        const image=new Image();
        if(d.type==='backdrop') {
          const under=document.createElement('canvas');under.width=1268;under.height=952;const paint=under.getContext('2d');paint.scale(2,2);paint.fillStyle='#fff';paint.fillRect(0,0,634,476);
          for(const n of d.under) {
            paint.save();paint.globalAlpha=n.opacity;paint.beginPath();paint.rect(...n.clip);paint.clip();
            if(n.kind==='box') {
              paint.beginPath();paint.roundRect(...n.bounds,n.radii);paint.fillStyle=n.color;paint.fill();
              const [x,y,w,h]=n.bounds,[top,right,bottom,left]=n.border;paint.fillStyle=n.borderColor;
              if(top)paint.fillRect(x,y,w,top);if(right)paint.fillRect(x+w-right,y,right,h);if(bottom)paint.fillRect(x,y+h-bottom,w,bottom);if(left)paint.fillRect(x,y,left,h);
            } else if(n.kind==='image'&&n.decoration)paint.drawImage(await raster(n.decoration),...n.bounds);
            paint.restore();
          }
          const filtered=document.createElement('canvas');filtered.width=1268;filtered.height=952;const blur=filtered.getContext('2d');
          blur.filter=d.filter.replace(/blur\(([\d.]+)px\)/g,(_,n)=>`blur(${Number(n)*2}px)`);blur.drawImage(under,0,0);
          ctx.beginPath();ctx.roundRect(0,0,d.width,d.height,d.radii);ctx.clip();ctx.drawImage(filtered,d.bounds[0]*2,d.bounds[1]*2,d.width*2,d.height*2,0,0,d.width,d.height);
        } else if(d.type==='photo') {
          image.src=photo||d.src;image.crossOrigin='anonymous';await image.decode();ctx.beginPath();ctx.roundRect(0,0,d.width,d.height,Math.min(d.radius,Math.min(d.width,d.height)/2));ctx.clip();
          const [x,y,w,h]=d.imgBounds;
          if(d.objectFit==='cover') {const scale=Math.max(w/image.width,h/image.height),sw=w/scale,sh=h/scale;ctx.drawImage(image,(image.width-sw)/2,(image.height-sh)/2,sw,sh,x,y,w,h);}
          else ctx.drawImage(image,x,y,w,h);
        } else {
          let source;
          if(d.type==='svg') {
            const svg=new DOMParser().parseFromString(d.html,'image/svg+xml').documentElement;
            svg.setAttribute('width',d.width);svg.setAttribute('height',d.height);svg.style.width=d.width+'px';svg.style.height=d.height+'px';source=new XMLSerializer().serializeToString(svg);
          } else {
            const svg=document.createElementNS('http://www.w3.org/2000/svg','svg');svg.setAttribute('width',d.width);svg.setAttribute('height',d.height);
            const w=d.cssWidth||d.width,h=d.cssHeight||d.height;svg.setAttribute('viewBox',`0 0 ${w} ${h}`);
            const foreign=document.createElementNS(svg.namespaceURI,'foreignObject');foreign.setAttribute('width',w);foreign.setAttribute('height',h);
            const div=document.createElement('div');div.setAttribute('xmlns','http://www.w3.org/1999/xhtml');Object.assign(div.style,{width:w+'px',height:h+'px',...d.style});if(d.type==='html')div.innerHTML=d.html;foreign.append(div);svg.append(foreign);source=new XMLSerializer().serializeToString(svg);
          }
          image.src='data:image/svg+xml;base64,'+btoa(unescape(encodeURIComponent(source)));await image.decode();ctx.drawImage(image,0,0,d.width,d.height);
        }
        return canvas;
      }
      return (await raster(d)).toDataURL('image/png').split(',')[1];
    },{d,photo});
    await fs.writeFile(file,Buffer.from(encoded,'base64'));
  }
  const font=await fetch('https://glimm.dev/_next/static/media/Inter_VariableFont-s.p.0oy~rb7-bgeiw.ttf');
  if(!font.ok)throw new Error('font '+font.status);
  await fs.writeFile(path.join(out,'Inter.ttf'),Buffer.from(await font.arrayBuffer()));
  const stylesheet=await fs.readFile('glimm-reference/0l_0jbw4u0n41.css','utf8');
  const spring=stylesheet.match(/__toast\{--spring:linear\(([^)]+)\)/)[1].split(',').map(Number);
  await fs.writeFile(path.join(out,'motion.rs'),`pub const TOAST_SPRING: &[f32] = &[${spring.map(v=>Number.isInteger(v)?v+'.0':v).join(',')}];\n`);
  await fs.writeFile(path.join(out,'images.rs'),`pub const IMAGES: &[(&str, &[u8])] = &[\n${[...decorations.keys()].sort().map(name=>`    ("${name}", include_bytes!("${name}")),`).join('\n')}\n];\n`);
  await fs.writeFile(path.join(out,'provenance.json'),JSON.stringify({source:'https://glimm.dev/#demos',package:'glimm@0.4.1',viewport:[634,476],description:'DOM layout measurements for native GPUI primitives. PNGs contain only original decorative CSS backgrounds, icons and cropped original photographs; no page or component screenshots are used by the native application.',scenes:Object.keys(scenes),decorations:decorations.size},null,2));
  console.log('Exported',Object.keys(scenes).length,'states and',decorations.size,'decorations');
} finally {await browser.close();}
