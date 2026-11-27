import json,re,statistics,subprocess,sys,time
from pathlib import Path
d=Path(__file__).parent
todo,bp,canvas,control,external=sys.argv[1:]
truth=json.loads((d/'evidence/oracle.json').read_text())
truth['external']={'pairs':[['/invoices','/invoices/add'],['/invoices','/invoices/:id/edit'],['/invoices/add','/invoices'],['/invoices/:id/edit','/invoices']]}
methods=['layout','referenced','reachable'];data={}
for i in range(8):
 for mode in methods[i%3:]+methods[:i%3]:
  start=time.monotonic_ns();p=subprocess.run(['/usr/bin/time','-l','node',str(d/'run.mjs'),mode,todo,bp,canvas,external],capture_output=True,text=True,check=True)
  ms=(time.monotonic_ns()-start)/1e6;rss=int(re.search(r'(\d+)\s+maximum resident set size',p.stderr)[1])/1048576
  pairs=json.loads(p.stdout);row=data.setdefault(mode,{'samples':[],'pairs':pairs})
  assert row['pairs']==pairs
  if i:row['samples'].append([ms,rss])
for mode,row in data.items():
 row['medianMs'],row['medianMiB']=map(statistics.median,zip(*row['samples']))
 row['score']={}
 for app,v in row['pairs'].items():
  actual=set(map(tuple,truth[app]['pairs']));found={tuple(json.loads(k)) for k in v['pairs']};row['score'][app]=[len(actual&found),len(found-actual),len(actual-found)]
 result=json.loads(subprocess.check_output(['node',str(d/'run.mjs'),mode,todo,control,canvas]))
 row['falseControls']=[to for to in ['/auth/register','/app/users','/app/profile'] if json.dumps(['/',to],separators=(',',':')) in result['bp']['pairs']]
(d/'evidence/reach.json').write_text(json.dumps(data,indent=2))
