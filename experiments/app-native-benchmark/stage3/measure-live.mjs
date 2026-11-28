#!/usr/bin/env node
import { spawn, execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import { createRequire } from "node:module";
const shared = createRequire(import.meta.url);
const { chromium } = shared("playwright-core");
const repo = path.resolve(path.dirname(new URL(import.meta.url).pathname), "../../..");
const out = path.resolve(process.argv[2] || "/tmp/dui-stage3-live");
const dui = process.argv[3] || path.join(repo, "target/debug/dui");
const chrome = process.env.CHROME_PATH || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const targets = {
  calcom: {
    root: process.env.CALCOM_WEB_ROOT,
    entry: "modules/event-types/views/event-types-listing-view.tsx",
    file: "modules/event-types/views/event-types-listing-view.tsx",
    before: "  return (\n    <InfiniteScrollMain",
    after: "  return (\n    <>Benchmark save marker {n}<InfiniteScrollMain",
    close: "    </InfiniteScrollMain>\n  );",
    closeAfter: "    </InfiniteScrollMain></>\n  );",
    probe: "Benchmark save marker {n}",
  },
  plane: {
    root: process.env.PLANE_WEB_ROOT,
    entry: "app/(all)/[workspaceSlug]/(projects)/projects/(detail)/[projectId]/issues/(list)/page.tsx",
    file: "app/(all)/[workspaceSlug]/(projects)/projects/(detail)/[projectId]/issues/(list)/page.tsx",
    before: '<div className="h-full w-full">',
    after: '<div className="h-full w-full">Benchmark save marker {n}',
    probe: "Benchmark save marker {n}",
  },
  obora: {
    root: process.env.OBORA_WEB_ROOT,
    entry: "src/features/organization.tsx",
    file: "src/features/organization.tsx",
    before: '  return (\n    <div className="mx-auto w-full max-w-5xl space-y-6">',
    after: '  return (\n    <div className="mx-auto w-full max-w-5xl space-y-6">Benchmark save marker {n}',
    probe: "Benchmark save marker {n}",
  },
};
const iterations = 10, deadlineMs = 2000;
const viewports = [{id:"desktop",width:1440,height:920},{id:"mobile",width:390,height:844}];
function copy(src,dst){fs.rmSync(dst,{recursive:true,force:true});fs.cpSync(src,dst,{recursive:true,filter:s=>!/(^|[\/])(node_modules|\.git|\.next|dist|build|\.output)([\/]|$)/.test(s)})}
function pct(a,p){if(!a.length)return null;return a[Math.min(a.length-1,Math.ceil(p/100*a.length)-1)]}
async function urlOf(child){return new Promise((ok,no)=>{let s="";const t=setTimeout(()=>no(Error("serve timeout")),8000);child.stdout.on("data",c=>{s+=c;const m=s.match(/http:\/\/127\.0\.0\.1:\d+/);if(m){clearTimeout(t);ok(m[0])}});child.on("exit",()=>no(Error("serve exited")))})}
fs.rmSync(out,{recursive:true,force:true});fs.mkdirSync(out,{recursive:true});
const results={deadlineMs,iterations,targets:{}};
const browser=await chromium.launch({executablePath:chrome,headless:true});
for(const [key,t] of Object.entries(targets)){
 const row=results.targets[key]={viewports:{}};
 try{
  if(!t.root) throw Error("root env missing");
  const work=path.join(out,key),project=path.join(work,"source"),home=path.join(work,"home");fs.mkdirSync(work,{recursive:true});copy(t.root,project);fs.mkdirSync(home);
  const file=path.join(project,t.file),original=fs.readFileSync(file,"utf8");
  if(!original.includes(t.before))throw Error("needle missing");
  const env={...process.env,HOME:home,DUI_CHROMIUM:"/missing-chrome"};
  const mapped=JSON.parse(execFileSync(dui,["map",project,"--entry",t.entry],{env,encoding:"utf8",maxBuffer:32*1024*1024}));
  const server=spawn(dui,["serve",mapped.output],{env,stdio:["ignore","pipe","pipe"]});const contexts=[];
  try{
   const url=await urlOf(server);
   for(const vp of viewports){const c=await browser.newContext({viewport:{width:vp.width,height:vp.height},deviceScaleFactor:1});contexts.push(c);const p=await c.newPage();await p.goto(url);await p.waitForSelector(".approximate-model-content",{timeout:10000}).catch(()=>{});row.viewports[vp.id]={samples:[]}}
   for(let i=0;i<iterations;i++){
    const marker=t.after.replaceAll("{n}",String(i)),expected=t.probe.replaceAll("{n}",String(i));
    let next=original.replace(t.before,marker);if(t.close)next=next.replace(t.close,t.closeAfter);
    fs.writeFileSync(file,next);const started=performance.now();
    await Promise.all(contexts.map(async(c,vi)=>{const vp=viewports[vi],p=c.pages()[0],sample={iteration:i,status:"deadline",ms:null};try{await p.waitForFunction(e=>[...document.querySelectorAll(".approximate-model-content")].some(x=>x.shadowRoot?.textContent?.includes(e)),expected,{timeout:deadlineMs,polling:"raf"});await p.evaluate(()=>new Promise(ok=>requestAnimationFrame(()=>requestAnimationFrame(ok))));sample.ms=Math.round((performance.now()-started)*10)/10;sample.status="reflected"}catch{}row.viewports[vp.id].samples.push(sample)}));
   }
   for(const vp of viewports){const ss=row.viewports[vp.id].samples,ok=ss.filter(s=>s.status==="reflected").map(s=>s.ms).sort((a,b)=>a-b);Object.assign(row.viewports[vp.id],{attempted:ss.length,reflectedCount:ok.length,missed:ss.length-ok.length,medianMs:pct(ok,50),p95Ms:pct(ok,95)})}
  }finally{for(const c of contexts)await c.close().catch(()=>{});server.kill("SIGTERM")}
 }catch(e){row.unavailable=String(e).slice(0,500)}
 fs.writeFileSync(path.join(out,"live.partial.json"),JSON.stringify(results,null,2));
}
await browser.close();fs.writeFileSync(path.join(out,"live.json"),JSON.stringify(results,null,2));console.log(JSON.stringify(results.targets,null,2));
