"""Three rotated fresh processes per scenario/method (one excluded warmup); built inputs are cached."""
import json,os,re,statistics,subprocess,sys,time
from pathlib import Path
root=Path(sys.argv[1]);dui=sys.argv[2];here=Path(__file__).parent
results=json.loads((root/'all.json').read_text())['cases'];samples={}
env={**os.environ,'HOME':str(root/'home'),'DUI_CHROMIUM':'/missing-chrome'}
for name,r in results.items():
 d=root/name;css=bool((d/'built.css').stat().st_size)
 methods={'map':[dui,'map',str(d)],'sheet':[dui,'model',str(d),'App.tsx']+(['--built-css','built.css','--viewport-width','1440'] if css else []),'lower':['node',str(here/'project-one.mjs'),'lower',str(d),dui,'1440'],'sidecar':['node',str(here/'project-one.mjs'),'sidecar',str(d),dui,'1440']}
 seq=list(methods);row=samples.setdefault(name,{})
 for rep in range(4):
  for mode in seq[rep:]+seq[:rep]:
   t=time.monotonic_ns();p=subprocess.run(['/usr/bin/time','-l',*methods[mode]],env=env,stdout=subprocess.DEVNULL,stderr=subprocess.PIPE);ms=(time.monotonic_ns()-t)/1e6
   rss=int(re.search(rb'(\d+)\s+maximum resident set size',p.stderr)[1])/1048576
   if rep:row.setdefault(mode,[]).append([round(ms,3),round(rss,3),p.returncode])
(root/'cost.json').write_text(json.dumps(samples,indent=2))
