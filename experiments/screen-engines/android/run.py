"""Explicit official emulator capture. Use a disposable, dedicated adb serial."""
import io,json,os,re,subprocess,sys,time
from pathlib import Path
from PIL import Image,ImageChops
stage=Path(sys.argv[1]);serial=sys.argv[2];here=Path(__file__).parent
apk=next((stage/'android-probe/build/outputs/apk/debug').glob('*.apk'))
adb=[str(Path.home()/'Library/Android/sdk/platform-tools/adb'),'-s',serial]
app='sh.dui.fixture';out=stage/'android-evidence';out.mkdir(exist_ok=True)
def cmd(*args,**opts):return subprocess.check_output([*adb,*args],**opts)
def shell(*args):return cmd('shell',*args).decode().strip()
def launch(screen,ref):
    shell('am','force-stop',app)
    shell('am','start','-W','-n',app+'/.SettingsActivity','--es','screen',screen,'--ez','reference',str(ref).lower())
    time.sleep(1)
def shot():return cmd('exec-out','screencap','-p')
source=(here/'Examples.kt').read_text();file=out/'Source.kt'
results={'apkBytes':apk.stat().st_size,'screens':{}}
cmd('install','-r',str(apk))
try:
    shell('wm','density','160')
    for width,height in [(1440,920),(390,844)]:
        shell('wm','size',f'{width}x{height}');time.sleep(1)
        for screen in ['DetailScreen','ProfileScreen']:
            label=screen.removesuffix('Screen');launch(screen,True)
            ref=shot();(out/f'{screen}-reference-{width}.png').write_bytes(ref)
            image=Image.open(io.BytesIO(ref)).convert('RGB')
            assert image.size==(width,height)
            white=Image.new('RGB',image.size,'white')
            assert ImageChops.difference(image,white).crop((width//2-90,height//2-35,width//2+90,height//2+35)).getbbox()
            shell('run-as',app,'rm','-f','files/ack','files/model.json')
            launch(screen,False)
            row=results['screens'].setdefault(screen,{})[str(width)]={'analysis':[],'edits':[],'rasterChecks':[]}
            for i in range(9):
                start=time.perf_counter_ns();file.write_text(source.replace(f'Text("{label}")',f'Text("{label} edited")') if i%2 else source)
                p=subprocess.run(['/usr/bin/time','-l',sys.executable,str(here/'project.py'),str(file),screen,str(i)],capture_output=True,check=True)
                if i:row['analysis'].append({'ms':(time.perf_counter_ns()-start)/1e6,'rssBytes':int(re.search(rb'(\d+)\s+maximum resident set size',p.stderr)[1])})
                cmd('shell','run-as',app,'sh','-c',"'cat >files/new && mv files/new files/model.json'",input=p.stdout)
                while time.perf_counter_ns()-start<3e9:
                    reply=subprocess.run([*adb,'shell','run-as',app,'cat','files/ack'],capture_output=True).stdout.strip()
                    if reply==str(i).encode():break
                else:
                    if i==0:raise RuntimeError('initial model not drawn')
                    row['edits'].append(None);continue
                elapsed=(time.perf_counter_ns()-start)/1e6
                if i:row['edits'].append(elapsed)
                png=shot();drawn=Image.open(io.BytesIO(png)).convert('RGB')
                delta=ImageChops.difference(image,drawn);same=delta.getbbox() is None
                row['rasterChecks'].append(same if i%2==0 else not same)
                if i==0:
                    (out/f'{screen}-model-{width}.png').write_bytes(png);row['match']=same
            stats=shell('dumpsys','meminfo',app)
            match=re.search(r'TOTAL PSS:\s+([\d,]+).*?TOTAL RSS:\s+([\d,]+)',stats,re.S)
            if match: row['pssKiB'],row['rssKiB']=[int(x.replace(',','')) for x in match.groups()]
            (out/'matrix.json').write_text(json.dumps(results,indent=2))
finally:
    shell('wm','size','reset');shell('wm','density','reset')
