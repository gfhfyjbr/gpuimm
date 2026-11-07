import { spawnSync } from 'node:child_process';
import fs from 'node:fs';
import path from 'node:path';

const root=process.cwd();
const binary=process.env.GLIMM_DEMO_BIN ?? path.join(root,'gpui-fast/target/debug/examples/glimm');
const prefix=process.env.GLIMM_CAPTURE_PREFIX ?? 'glimm-native';
const resultsFile=process.env.GLIMM_SMOKE_RESULTS ?? 'glimm-demo-smoke.json';
const demos=['sheet','join','invoice','upgrade','reset','publish','autopilot','theme','tasks'];
const times=[3290,1100,980,1200,1320,2100,1000,1050,1050];
const cases=demos.flatMap((name,demo)=>[
  {name:`${name}-before`,demo,args:['--state','initial'],active:false},
  {name:`${name}-after`,demo,args:['--state','final'],active:false},
  {name:`${name}-transition`,demo,args:['--time',String(times[demo])],active:true},
]);
cases.push(
  {name:'sheet-gather',demo:0,args:['--gather','--time','1600'],active:true},
  {name:'sheet-band',demo:0,args:['--config',path.join(root,'glimm-band-settings.json'),'--time','3290'],active:true},
  {name:'sheet-custom',demo:0,args:['--config',path.join(root,'glimm-settings.json'),'--time','3290'],active:true},
  {name:'desktop',demo:0,args:['--time','3290'],active:true,desktop:true},
);
const results=[];
for(const test of cases){
  const file=path.join(root,`${prefix}-${test.name}.png`);
  const args=[...(test.desktop?[]:['--scene-only']),'--capture',file,'--demo',String(test.demo),...test.args];
  const started=performance.now();
  const run=spawnSync(binary,args,{cwd:path.join(root,'gpui-fast'),encoding:'utf8',timeout:30000});
  const exists=fs.existsSync(file);
  const active=run.stderr?.includes(`active=${test.active}`);
  const ok=run.status===0&&exists&&active;
  const result={name:test.name,ok,exit:run.status,active:test.active,file:path.basename(file),bytes:exists?fs.statSync(file).size:0,ms:Math.round(performance.now()-started),diagnostics:run.stderr?.trim()};
  results.push(result);console.log(`${ok?'PASS':'FAIL'} ${test.name} ${result.bytes} bytes (${result.ms}ms)`);
}
fs.writeFileSync(resultsFile,JSON.stringify(results,null,2)+'\n');
if(results.some(r=>!r.ok))process.exitCode=1;
