"""Opt-in, two literal SwiftUI Text screens, host save through simulator acknowledgement."""
import hashlib, json, os, re, subprocess, sys, time
from pathlib import Path
stage=Path(sys.argv[1]); docs=Path((stage/(sys.argv[2] if len(sys.argv)>2 else 'container')).read_text().strip())/'Documents'
docs.mkdir(exist_ok=True); source=Path('fixtures/swiftui-map/Sources/SwiftUIMapFixture/Screens.swift').read_text()
fixture=stage/'Edited.swift'; output={}
for screen in ['DetailView','ProfileView']:
    name=screen.removesuffix('View'); row=output[screen]={'analysis':[],'edits':[]}
    for index in range(9):
        text=name+(' edited' if index%2 else '')
        start=time.perf_counter_ns()
        fixture.write_text(source.replace(f'Text("{name}")',f'Text("{text}")'))
        cmd=['/usr/bin/time','-l','xcrun','swiftc','-frontend','-dump-parse',str(fixture)]
        p=subprocess.run(cmd,capture_output=True,text=True,check=True)
        tree=p.stdout or p.stderr
        decl=next(block for block in tree.split('\n  (struct_decl ') if f'"{screen}" inherits=' in block.splitlines()[0])
        if decl.count('(call_expr ')!=1 or 'name="Text"' not in decl: raise ValueError('unsupported declaration')
        values=re.findall(r'string_literal_expr[^\n]+value="([^"]*)"',decl)
        if values!=[text]: raise ValueError(values)
        tmp=docs/'model.pending';tmp.write_text(json.dumps({'screen':screen,'text':values[0],'revision':index, 'viewportWidth':int(os.getenv('DUI_VIEWPORT','390'))}));os.replace(tmp,docs/'model.json')
        ms=(time.perf_counter_ns()-start)/1e6
        rss=int(re.search(r'(\d+)\s+maximum resident set size',p.stderr)[1]);ack=docs/'ack'
        if index:row['analysis'].append({'ms':ms,'rssBytes':rss})
        while time.perf_counter_ns()-start<3e9:
            if ack.exists() and ack.read_text()==str(index):break
            time.sleep(.004)
        else:
            if index==0:raise RuntimeError('initial model not acknowledged')
            row['edits'].append(None);continue
        if index:row['edits'].append((time.perf_counter_ns()-start)/1e6)
        if 'surface' in str(docs):
            for width in (390,1440):
                ref=(docs/f'{screen}-reference-{width}.png').read_bytes()
                candidate=(docs/f'{screen}-model-{width}.png').read_bytes()
                if (hashlib.sha256(ref).digest()==hashlib.sha256(candidate).digest()) != (index%2==0):
                    raise RuntimeError('raster failed to reflect edit')
            if index<8:
                procs=subprocess.check_output(['ps','-axo','rss=,comm='],text=True)
                sizes=[int(line.split()[0])*1024 for line in procs.splitlines() if line.rstrip().endswith('/SurfaceProbe')]
                if len(sizes)==1: row.setdefault('rendererRSS',[]).append(sizes[0])
        if device := os.getenv('DUI_PROBE_SIMULATOR'):
            from PIL import Image
            path=stage/f'{screen}-device-{os.environ["DUI_VIEWPORT"]}-{index}.png'
            subprocess.run(['xcrun','simctl','io',device,'screenshot',str(path)],check=True,capture_output=True)
            with Image.open(path) as image:
                w,h=image.size
                crop=image.convert('RGB').crop((w//2-220,h//2-180,w//2+220,h//2+180))
                colors=crop.getextrema()
                if all(hi-lo<20 for lo,hi in colors):raise ValueError('blank device crop')
                current=hashlib.sha256(crop.tobytes()).digest()
                if index==0: baseline=current
                elif (current==baseline)!=(index%2==0):raise ValueError('device crop failed to reflect edit')
(stage/(sys.argv[3] if len(sys.argv)>3 else 'native-matrix.json')).write_text(json.dumps(output,indent=2))
