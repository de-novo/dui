"""Benchmark the pinned macOS release probes. Supply the three specificity binaries and fixture."""
from pathlib import Path
import json,re,subprocess,sys,tempfile,time
here=Path(__file__).resolve().parent
original,simple,servo,sample=sys.argv[1:]
bin=here.parent/'renderer-geometry/target/release'
def measure(cmd,stdin=None):
 t=time.perf_counter_ns()
 p=subprocess.run(['/usr/bin/time','-l',*cmd],input=stdin,capture_output=True,check=True)
 ms=(time.perf_counter_ns()-t)/1e6
 return json.loads(p.stdout),[ms,int(re.search(rb'(\d+)\s+maximum resident set size',p.stderr)[1])/1048576]
result={'layout':{},'style':{}}
with tempfile.TemporaryDirectory() as tmp:
 root=Path(tmp);(root/'App.tsx').write_text('export function App(){return <div id="box"/>}')
 for case,css in {'fixed':'width:80px;height:24px','translated':'width:80px;height:24px;transform:translate(12px,6px)','constrained':'width:80px;height:24px;max-width:50px'}.items():
  (root/'built.css').write_text('#box{'+css+'}')
  model=subprocess.check_output([bin/'fixture-model',root]);methods=[('Taffy',[bin/'renderer-geometry']),('Yoga',['node',here/'yoga.mjs'])]
  group=result['layout'][case]={'css':css}
  for i in range(8):
   for k,cmd in methods[::1 if i%2==0 else -1]:
    v,s=measure(cmd,model);r=group.setdefault(k,{'rect':v['rect'],'samples':[]});assert r['rect']==v['rect']
    if i:r['samples'].append(s)
for i in range(8):
 methods=[('original',original),('simplecss',simple),('selectors',servo)]
 for k,exe in methods[i%3:]+methods[:i%3]:
  model,s=measure([exe,sample]);r=result['style'].setdefault(k,{'padding':[v['value'] for v in model['components'][0]['root']['style'] if v['property'].startswith('padding')],'samples':[]})
  if i:r['samples'].append(s)
(here/'evidence/engines.json').write_text(json.dumps(result,indent=2))
