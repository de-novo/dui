#!/usr/bin/env node
import { spawn, execFileSync } from "node:child_process";
import fs from "node:fs";
import path from "node:path";
import crypto from "node:crypto";
import { createRequire } from "node:module";
const shared = createRequire(import.meta.url);
const { chromium } = shared("playwright-core");
const here = path.dirname(new URL(import.meta.url).pathname);
const repo = path.resolve(here, "../../..");
const out = path.resolve(process.argv[2] || "/tmp/dui-stage3-live");
const dui = process.argv[3] || path.join(repo, "target/debug/dui");
const chrome = process.env.CHROME_PATH || "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome";
const fixtures = JSON.parse(fs.readFileSync(path.join(here, "fixtures.json"), "utf8"));
if (process.env.STAGE3_FIXTURE_OVERLAY) {
  const overlay = JSON.parse(fs.readFileSync(process.env.STAGE3_FIXTURE_OVERLAY, "utf8"));
  Object.assign(fixtures.targets, overlay.targets ?? {});
}
const targets = {
  calcom: {
    root: process.env.CALCOM_WEB_ROOT,
    entry: "modules/event-types/views/event-types-listing-view.tsx",
    file: "modules/event-types/views/event-types-listing-view.tsx",
    before: "  return (\n    <InfiniteScrollMain\n      profiles={userEventGroupsData.profiles}\n      eventTypeGroups={userEventGroupsData.eventTypeGroups}\n    />\n  );",
    after: "  return (\n    <>Benchmark save marker {n}\n    <InfiniteScrollMain\n      profiles={userEventGroupsData.profiles}\n      eventTypeGroups={userEventGroupsData.eventTypeGroups}\n    />\n    </>\n  );",
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
const iterations = 10, deadlineMs = 5000;
const liveState = process.env.STAGE3_LIVE_STATE || "populated";
const viewports = [{id:"desktop",width:1440,height:920},{id:"mobile",width:390,height:844}];
function copy(src,dst){fs.rmSync(dst,{recursive:true,force:true});fs.cpSync(src,dst,{recursive:true,filter:s=>!/(^|[\/])(node_modules|\.git|\.next|dist|build|\.output)([\/]|$)/.test(s)})}
function pct(a,p){if(!a.length)return null;return a[Math.min(a.length-1,Math.ceil(p/100*a.length)-1)]}
function stableJson(value){if(Array.isArray(value))return `[${value.map(stableJson).join(",")}]`;if(value&&typeof value==="object")return `{${Object.keys(value).sort().map(key=>`${JSON.stringify(key)}:${stableJson(value[key])}`).join(",")}}`;return JSON.stringify(value)??"null"}
function payloadHash(value){return crypto.createHash("sha256").update(stableJson(value)).digest("hex")}
async function urlOf(child){return new Promise((ok,no)=>{let s="";const t=setTimeout(()=>no(Error("serve timeout")),8000);child.stdout.on("data",c=>{s+=c;const m=s.match(/http:\/\/127\.0\.0\.1:\d+/);if(m){clearTimeout(t);ok(m[0])}});child.on("exit",()=>no(Error("serve exited")))})}
function fixtureAssignment(model,config,state){
 const wanted=config?.component;
 if(!wanted)return{values:{},missingInputs:[],querySlotsWithoutInput:[],component:undefined};
 const comp=model.components?.find(c=>c.name===wanted||c.id===wanted);
 if(!comp)throw Error(`configured component missing: ${wanted}`);
 const known=new Set((model.components??[]).flatMap(c=>(c.slots??[]).map(s=>s.id)));
 const values={},missingInputs=[];
 for(const [key,value] of Object.entries(config?.slots?.[state]??{})){
  if(key.includes("#")){
   values[key]=value;
   if(!known.has(key))missingInputs.push(key);
   continue;
  }
  const slot=(comp.slots??[]).find(s=>s.name===key);
  if(slot)values[slot.id]=value;
  else missingInputs.push(key);
 }
 const supplied=new Set(Object.keys(values));
 const querySlotsWithoutInput=(comp.slots??[]).filter(s=>s.kind==="query"&&!supplied.has(s.id)).map(s=>s.name);
 return{values,missingInputs,querySlotsWithoutInput,component:comp};
}
function modelCardProbe(title){
 const cards=[...document.querySelectorAll(".screen-card,.mobile-deck-card")];
 const card=cards.find(c=>c.dataset.selected==="true"||c.classList.contains("selected"))??cards.find(c=>c.getAttribute("title")===title||c.dataset.screenId===title)??cards[0];
 if(!card)return{found:false};
 const frame=[...card.querySelectorAll(".approximate-model-content")][0];
 const shadow=frame?.shadowRoot;
 const visibleText=shadow?[...shadow.children].filter(el=>el.tagName!=="STYLE").map(el=>el.textContent).join(" ").replace(/\s+/g," ").trim():"";
 return{found:true,selected:card.dataset.selected==="true"||card.classList.contains("selected"),title:card.getAttribute("title")??card.dataset.screenId,logicalWidth:frame?.dataset.logicalWidth,logicalHeight:frame?.dataset.logicalHeight,visibleTextLength:visibleText.length,visibleText};
}
function viewerUrl(url,screen,viewport,state){
 const next=new URL(url);
 next.searchParams.set("view","overview");
 next.searchParams.set("width",String(viewport.width));
 next.searchParams.set("height",String(viewport.height));
 if(screen)next.searchParams.set("screen",screen);
 if(state)next.searchParams.set("state",state);
 return String(next);
}
fs.rmSync(out,{recursive:true,force:true});fs.mkdirSync(out,{recursive:true});
const results={deadlineMs,iterations,liveState,targets:{}};
const browser=await chromium.launch({executablePath:chrome,headless:true});
for(const [key,t] of Object.entries(targets)){
 const row=results.targets[key]={viewports:{}};
 try{
  if(!t.root)throw Error("root env missing");
  const work=path.join(out,key),project=path.join(work,"source"),home=path.join(work,"home");fs.mkdirSync(work,{recursive:true});copy(t.root,project);fs.mkdirSync(home);
  const file=path.join(project,t.file),original=fs.readFileSync(file,"utf8");
  if(!original.includes(t.before))throw Error("needle missing");
  const env={...process.env,HOME:home,DUI_CHROMIUM:"/missing-chrome"};
  const extractedModel=JSON.parse(execFileSync(dui,["model",project,t.entry],{env,encoding:"utf8",maxBuffer:32*1024*1024}));
  const fixtureConfig=fixtures.targets?.[key]??{};
  const expectedTexts=fixtureConfig.assertions?.[liveState]??[];
  const assignment=fixtureAssignment(extractedModel,fixtureConfig,liveState);
  const inputStates={[liveState]:{
   slots:assignment.values,
   ...(fixtureConfig.expressions?.[liveState]?{expressions:fixtureConfig.expressions[liveState]}:{})
  }};
  fs.writeFileSync(path.join(project,"dui.source-inputs.json"),JSON.stringify({states:inputStates},null,2));
  const manifestPath=path.join(project,"dui.json");
  const manifest=fs.existsSync(manifestPath)?JSON.parse(fs.readFileSync(manifestPath,"utf8")):{};
  manifest.sourceInputs="dui.source-inputs.json";
  fs.writeFileSync(manifestPath,JSON.stringify(manifest,null,2));
  const mapped=JSON.parse(execFileSync(dui,["map",project,"--entry",t.entry],{env,encoding:"utf8",maxBuffer:32*1024*1024}));
  const archivedModel=JSON.parse(fs.readFileSync(path.join(mapped.output,"source-model.json"),"utf8"));
  const screen=archivedModel.screens?.find(s=>s.component===assignment.component?.id)??archivedModel.screens?.[0];
  const title=screen?.id??assignment.component?.name??"";
  row.fixture={state:liveState,payloadSha256:payloadHash({slots:fixtureConfig?.slots?.[liveState]??{},expressions:fixtureConfig?.expressions?.[liveState]??{}}),missingInputs:assignment.missingInputs,querySlotsWithoutInput:assignment.querySlotsWithoutInput,component:assignment.component?.id,screen:screen?.id,configuredSlotCount:Object.keys(assignment.values).length,productionInputPath:"dui.source-inputs.json",transportInjection:false};
  const server=spawn(dui,["serve",mapped.output],{env,stdio:["ignore","pipe","pipe"]});const contexts=[];
  try{
   const url=await urlOf(server);
   for(const vp of viewports){
    const c=await browser.newContext({viewport:{width:vp.width,height:vp.height},deviceScaleFactor:1});
    contexts.push(c);
    const p=await c.newPage();
    await p.goto(viewerUrl(url,screen?.id,vp,liveState));
    await p.waitForSelector(".approximate-model-content",{timeout:10000}).catch(()=>{});
    await p.waitForFunction(({title,expectedTexts})=>{
     const cards=[...document.querySelectorAll(".screen-card,.mobile-deck-card")];
     const card=cards.find(c=>c.dataset.selected==="true"||c.classList.contains("selected"))??cards.find(c=>c.getAttribute("title")===title||c.dataset.screenId===title)??cards[0];
     const frame=card?[...card.querySelectorAll(".approximate-model-content")][0]:undefined;
     const shadow=frame?.shadowRoot;
     if(!shadow)return false;
     const text=[...shadow.children].filter(el=>el.tagName!=="STYLE").map(el=>el.textContent).join(" ");
     return expectedTexts.every(expected=>text.includes(expected));
    },{title,expectedTexts},{timeout:5000,polling:"raf"}).catch(()=>{});
    const card=await p.evaluate(modelCardProbe,title);
    row.viewports[vp.id]={samples:[],card,missingText:expectedTexts.filter(text=>!card.visibleText?.includes(text)),assertedTextCount:expectedTexts.length};
   }
   for(let i=0;i<iterations;i++){
    const marker=t.after.replaceAll("{n}",String(i)),expected=t.probe.replaceAll("{n}",String(i));
    let next=original.replace(t.before,marker);if(t.close)next=next.replace(t.close,t.closeAfter);
    if(!next.includes(expected)||next===original)throw Error("controlled edit did not apply");fs.writeFileSync(file,next);const started=performance.now();
    await Promise.all(contexts.map(async(c,vi)=>{
     const vp=viewports[vi],p=c.pages()[0],sample={iteration:i,status:"deadline",ms:null};
     try{
      await p.waitForFunction(({expected,title,expectedTexts})=>{
       const cards=[...document.querySelectorAll(".screen-card,.mobile-deck-card")];
       const card=cards.find(c=>c.dataset.selected==="true"||c.classList.contains("selected"))??cards.find(c=>c.getAttribute("title")===title||c.dataset.screenId===title)??cards[0];
       const frame=card?[...card.querySelectorAll(".approximate-model-content")][0]:undefined;
       const shadow=frame?.shadowRoot;
       if(!shadow)return false;
       const text=[...shadow.children].filter(el=>el.tagName!=="STYLE").map(el=>el.textContent).join(" ");
       return text.includes(expected)&&expectedTexts.every(fixtureText=>text.includes(fixtureText));
      },{expected,title,expectedTexts},{timeout:deadlineMs,polling:"raf"});
      await p.evaluate(()=>new Promise(ok=>requestAnimationFrame(()=>requestAnimationFrame(ok))));
      sample.ms=Math.round((performance.now()-started)*10)/10;sample.status="reflected";
     }catch{}
     row.viewports[vp.id].samples.push(sample);
    }));
   }
   for(const vp of viewports){const ss=row.viewports[vp.id].samples,ok=ss.filter(s=>s.status==="reflected").map(s=>s.ms).sort((a,b)=>a-b);Object.assign(row.viewports[vp.id],{attempted:ss.length,reflectedCount:ok.length,missed:ss.length-ok.length,medianMs:pct(ok,50),p95Ms:pct(ok,95)})}
  }finally{
   for(const c of contexts)await c.close().catch(()=>{});
   server.kill("SIGTERM");
  }
 }catch(e){row.unavailable=String(e).slice(0,500)}
 fs.writeFileSync(path.join(out,"live.partial.json"),JSON.stringify(results,null,2));
}
await browser.close();fs.writeFileSync(path.join(out,"live.json"),JSON.stringify(results,null,2));console.log(JSON.stringify(results.targets,null,2));
