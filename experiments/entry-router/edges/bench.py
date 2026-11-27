"""Measure one fresh Node process per three-app analysis; rotate execution order."""
import json,re,subprocess,statistics,sys,time
from pathlib import Path
base=Path(__file__).resolve().parent
todo,bp,canvas,control=sys.argv[1:]
truth=json.loads((base/'evidence/oracle.json').read_text())
data={}
methods=['local','imports','layout']
for i in range(8):
 for m in methods[i%3:]+methods[:i%3]:
  start=time.monotonic_ns()
  p=subprocess.run(['/usr/bin/time','-l','node',str(base/'run.mjs'),m,todo,bp,canvas],capture_output=True,text=True,check=True)
  wall=(time.monotonic_ns()-start)/1e6
  rss=int(re.search(r'(\d+)\s+maximum resident set size',p.stderr)[1])/1048576
  result=json.loads(p.stdout)
  if not i:
   entry=data[m]={'samples':[],'pairs':result}
   continue
  assert result==data[m]['pairs']
  data[m]['samples'].append([wall,rss])
for m,d in data.items():
 d['medianMs'],d['medianMiB']=map(statistics.median,zip(*d['samples']))
 d['methodLOC']=sum(bool(x.strip()) for x in (base/(m+'.mjs')).read_text().splitlines())
 scores={}
 for app,a in d['pairs'].items():
  actual=set(map(tuple,truth[app]['pairs']));found={tuple(json.loads(k)) for k in a['pairs']}
  scores[app]=[len(found&actual),len(found-actual),len(actual-found)]
 d['scores_TP_FP_FN']=scores
 ctrl=json.loads(subprocess.check_output(['node',str(base/'run.mjs'),m,todo,control,canvas]))['bp']['pairs']
 d['negativeControlFalseEdges']=[pair for pair in [['/','/auth/register'],['/','/app/users']] if json.dumps(pair,separators=(',',':')) in ctrl]
(base/'evidence/results.json').write_text(json.dumps(data,indent=2))
