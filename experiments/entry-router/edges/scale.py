"""Supply pinned audit root (oss/, oss-results.json, oss-out/) and output JSON."""
from pathlib import Path
import json,os,time,subprocess,statistics,re,sys
base=Path(sys.argv[1]);out=Path(sys.argv[2]);here=Path(__file__).parent
audit={r['name']:r for r in json.loads((base/'oss-results.json').read_text())}
apps=['shadcn-admin','next-shadcn-admin','shadcn-store'];modes=['local','render','data'];results={}
for rep in range(8):
 for a in apps[rep%3:]+apps[:rep%3]:
  root=base/'oss'/a
  for mode in modes[rep%3:]+modes[:rep%3]:
   t=time.monotonic_ns();p=subprocess.run(['/usr/bin/time','-l','node',here/'scale.mjs',root,mode],text=True,capture_output=True,check=True);ms=(time.monotonic_ns()-t)/1e6
   graph=json.loads(p.stdout);row=results.setdefault(a,{}).setdefault(mode,{'graph':graph,'samples':[]});assert graph==row['graph']
   if rep:row['samples'].append([ms,int(re.search(r'(\d+)\s+maximum resident',p.stderr)[1])/1048576])
for a in apps:
 row=results[a];root=base/'oss'/a;files=[]
 for dir,subs,names in os.walk(root):
  subs[:]=[s for s in subs if not s.startswith('.') and s not in ['dist','node_modules','build','coverage','out']]
  files.extend(Path(dir)/s for s in names if s.endswith(('.ts','.tsx')) and not s.endswith('.d.ts'))
 row['sourceSize']=[len(files),sum(len(f.read_bytes().splitlines()) for f in files)]
 auditrow=audit[a];checked=set(auditrow['actualScreenPaths']);truth=set(map(tuple,auditrow['actualEdgePairs']));decl=set(auditrow['declaredRoutes'])
 row['oracle']={'declared':sorted(decl),'chromePages':sorted(checked),'chromePairs':sorted(truth)}
 for mode in modes:
  r=row[mode];r['medianMs'],r['medianMiB']=map(statistics.median,zip(*r['samples']));pred={tuple(p) for p in r['graph']['pairs'] if set(p)<=checked};screens=set(r['graph']['screens']);r['scores']=[len(screens&decl),len(decl),len(screens-decl),len(checked&screens),len(checked),len(pred&truth),len(truth),len(pred-truth)]
out.write_text(json.dumps(results,indent=2))
for a in apps:
 print(a,results[a]['sourceSize'],[(m,results[a][m]['scores'],round(results[a][m]['medianMs'],1),round(results[a][m]['medianMiB'],1)) for m in modes])
