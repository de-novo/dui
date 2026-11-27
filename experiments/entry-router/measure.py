"""macOS benchmark: one warm-up, seven fresh processes, rotating application order."""
import sys,time,json,re,subprocess,statistics
from pathlib import Path
if len(sys.argv) != 4:
    sys.exit("usage: python measure.py TODO_PROJECT CANVAS_PROJECT BULLETPROOF_PROJECT")
probe = Path(__file__).with_name("discover.mjs")
apps=[('TodoMVC',Path(sys.argv[1]),'js/app.tsx'),('dui canvas',Path(sys.argv[2]),'src/preview.tsx'),('Bulletproof React',Path(sys.argv[3]),'src/main.tsx')]
results={n:[] for n,*_ in apps}
for i in range(8):
 for name,root,entry in apps[i%3:]+apps[:i%3]:
  start=time.perf_counter_ns()
  p=subprocess.run(['/usr/bin/time','-l','node',str(probe),str(root),entry],capture_output=True,text=True,check=True)
  wall=(time.perf_counter_ns()-start)/1e6
  rss=int(re.search(r'(\d+)\s+maximum resident set size',p.stderr)[1])/1048576
  if i: results[name].append((wall,rss,p.stdout.strip()))
for name,values in results.items():
 assert len({x[2] for x in values})==1
 print(name,statistics.median(v[0] for v in values),statistics.median(v[1] for v in values),values[0][2])
