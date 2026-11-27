"""Produce scored tables and paired screenshot evidence; requires Pillow."""
import json,statistics,sys,zipfile
from pathlib import Path
from PIL import Image,ImageDraw
root=Path(sys.argv[1]);out=Path(__file__).parent/'evidence';out.mkdir(exist_ok=True)
data=json.loads((root/'all.json').read_text());cases=data['cases'];cost=json.loads((root/'cost.json').read_text());ids=list(dict.fromkeys(r['id'] for r in cases.values()));modes=['map','sheet','lower','sidecar']
assert len(ids)==44 and len(cases)==56
official=set(ids[:37]);live=json.loads((root/'live.json').read_text());assert len(live)==44
groups={'Layout':['block','inline','flex','grid','position','overflow'],'Sizing':['percent','rem','em','vh','vw'],'Text':['text-wrap','font'],'Paint, assets and transforms':['inherit-style','cascade','custom-property','color','border','radius','image','icon','translate','transform-order'],'Native controls':['input','checkbox','select','button'],'Style translation':['inline-style','class-composition','clsx','tailwind','stylesheet','css-module','media-query'],'JSX translation':['and','ternary','map-key','fragment','props','children','default-prop','spread','expression-text','initial-state']}
ids=[i for group in groups.values() for i in group];assert len(set(ids))==44
def rows(id):return [(k,r) for k,r in cases.items() if r['id']==id]
def passed(id,mode,w):return all(not r['views'][str(w)][mode]['errors'] for _,r in rows(id))
def fmt(i,m):return ' / '.join('✓' if passed(i,m,w) else '×' for w in [1440,390])
def number(a):return f'{a:.2f}'.rstrip('0').rstrip('.')
def explain(i,w):
 errors=[(k,e) for _,r in rows(i) for k,e in r['views'][str(w)]['map']['errors'].items()]
 if not errors:return '—'
 distinct=[]
 for k,e in errors:
  if isinstance(e,(int,float)):s=k+' Δ'+number(e)
  elif isinstance(e,str):s=k+' '+e
  else:s=k+' differs'
  if s not in distinct:distinct.append(s)
 return ', '.join(distinct)
totals={m:[sum(passed(i,m,w) for i in official) for w in [1440,390]]+[sum(all(passed(i,m,w) for w in [1440,390]) for i in official)] for m in modes}
lines=['# Fixed web basics baseline','','Main 9f622bae075cf669ad82e859fa9567556eed86dd. Chrome '+data['chrome']+', macOS 26.5.2 arm64 / Apple M1 Pro, en-US, DPR 1. Every scenario and mandatory probe must pass. Columns show desktop / mobile. Missing elements and rejected experimental inputs count as failures.','','| Approach | 1440×920 | 390×844 | Both |','|---|---:|---:|---:|']+['| '+m+' | '+' | '.join(f'{v}/37' for v in totals[m])+' |' for m in modes]+['','| Fixture | Scenarios | map | sheet | lower | sidecar | Desktop baseline errors | Mobile baseline errors |','|---|---:|---|---|---|---|---|---|']
lines=lines[:-2]
def hot(i):
 def one(v):return f'{v["ms"]:.1f} ms' if v['status']=='reflected' else ('Already equal' if v['status']=='unchanged-already-equal' else '>2 s')
 return ' / '.join(one(v) for v in live[i]['trials'])
for g,items in groups.items():
 lines+=['','## '+g,'','| Fixture | Scenarios | map | sheet | lower | sidecar | Desktop default errors | Mobile default errors | Live edit desktop / mobile |','|---|---:|---|---|---|---|---|---|---|']
 for i in items:lines.append('| '+' | '.join([i+(' (supplement)' if i not in official else ''),str(len(rows(i))),*(fmt(i,m) for m in modes),explain(i,1440),explain(i,390),hot(i)])+' |')
lines+=['','## Analysis cost','','Each cell is median wall ms / peak process RSS MiB across the three measured fresh processes per scenario, pooled within each ID. One initial run per method/scenario discarded. Costs use prebuilt fixture outputs at width 1440. Map includes archive writes; sheet emits a model; lower includes Node startup, projection and a Rust child; sidecar emits the isolated experimental representation. Process RSS is not simultaneous process-tree RSS. A rejected attempt is included with its exit status in cost.json.','','| ID | map | sheet | lower | sidecar |','|---|---:|---:|---:|---:|']
for i in ids:
 vals=[]
 for mode in modes:
  s=[sample for name,_ in rows(i) for sample in cost[name][mode]];vals.append(f'{statistics.median(v[0] for v in s):.1f} / {statistics.median(v[1] for v in s):.1f}')
 lines.append('| '+' | '.join([i,*vals])+' |')
(out/'baseline.md').write_text('\n'.join(lines)+'\n');(out/'summary.json').write_text(json.dumps(totals,indent=2))
for w in [1440,390]:
 img=Image.new('RGB',(1000,len(ids)*32+75),'white');draw=ImageDraw.Draw(img)
 for x,title in [(12,'Fixture'),(175,'Chrome'),(375,'Default card'),(575,'Built sheet'),(775,'Static scene')]:draw.text((x,8),title,fill='#154835',font_size=18)
 for j,i in enumerate(ids):
  name,r=next(((k,r) for k,r in rows(i) if r['views'][str(w)]['map']['errors']),rows(i)[0]);y=40+j*32
  draw.text((10,y+3),i,fill='#146749' if passed(i,'map',w) else '#993725',font_size=14)
  for x,m in [(175,'Chrome'),(375,'map'),(575,'sheet'),(775,'sidecar')]:
   p=root/name/f'{m}-{w}.png'
   if p.exists():
    panel=Image.open(p).convert('RGB').crop((0,0,270,40));panel.thumbnail((188,28));img.paste(panel,(x,y))
   else:draw.text((x,y),'Extraction rejected',fill='#993725',font_size=12)
 draw.text((12,img.height-28),f'{w}px card | One representative scenario per ID; score covers all scenarios.',fill='#154835',font_size=15);img.save(out/f'basics-{w}.png')
with zipfile.ZipFile(out/'paired-captures.zip','w',zipfile.ZIP_DEFLATED) as z:
 for name in cases:
  for p in sorted((root/name).glob('*.png')):z.write(p,name+'/'+p.name)
with zipfile.ZipFile(out/'models.zip','w',zipfile.ZIP_DEFLATED) as z:
 for name in cases:
  for f in ['App.tsx','built.css','baseline.json','sidecar.json','bindings.json']:z.write(root/name/f,name+'/'+f)
for f in ['cost.json','live.json']:(out/f).write_bytes((root/f).read_bytes())
(out/'results.json').write_bytes((root/'all.json').read_bytes())
print(totals)
