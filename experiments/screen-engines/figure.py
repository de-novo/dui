"""Compose the actual captured screens into one contact sheet per logical width."""
from pathlib import Path
import sys
from PIL import Image, ImageDraw, ImageFont
stage=Path(sys.argv[1]); out=Path(__file__).parent/'evidence'
font=ImageFont.truetype('/System/Library/Fonts/Helvetica.ttc',18)
for w,h in [(1440,920),(390,844)]:
    width=425 if w==1440 else 225
    height=round(h*width/w); row=height+38
    sheet=Image.new('RGB',(3*(width+8)+8,44+5*row),'#dbe2ea')
    d=ImageDraw.Draw(sheet)
    for col,title in enumerate(['Chrome reference','dui DOM','Yoga + canvas text']):
        d.text((8+col*(width+8),12),title,font=font,fill='#192332')
    for index,slug in enumerate(['403','503','workspace','inline','grid']):
        y=44+index*row;d.text((8,y),{'403':'shadcn-admin /403','503':'shadcn-admin /503','workspace':'dui EmptyWorkspace','inline':'#441 Inline','grid':'#441 Grid'}[slug],font=font,fill='#192332')
        for col,engine in enumerate(['Chrome','DOM','Yoga']):
            img=Image.open(stage/'web'/slug/f'{engine}-{w}.png').convert('RGB')
            assert img.size==(w,h)
            sheet.paste(img.resize((width,height),Image.Resampling.LANCZOS),(8+col*(width+8),y+26))
    sheet.save(out/f'web-{w}.png',optimize=True)
from PIL import ImageChops
from zipfile import ZipFile,ZIP_DEFLATED
native=stage/'repeat/surface/Documents'
with ZipFile(out/'full-screens.zip','w',ZIP_DEFLATED) as archive:
    for path in sorted((stage/'web').glob('*/*.png')):archive.write(path,'web/'+path.parent.name+'/'+path.name)
    for path in sorted(native.glob('*.png')):archive.write(path,'ios/'+path.name)
sheet=Image.new('RGB',(900,360),'#dbe2ea');d=ImageDraw.Draw(sheet)
for col,title in enumerate(['SwiftUI reference','Model → SwiftUI Text']):
    d.text((8+450*col,10),title,font=font,fill='#192332')
for row,(name,w) in enumerate([(s,w) for s in ['DetailView','ProfileView'] for w in [1440,390]]):
    y=40+row*78
    for col,mode in enumerate(['reference','model']):
        img=Image.open(native/f'{name}-{mode}-{w}.png').convert('RGB'); box=ImageChops.difference(img,Image.new('RGB',img.size,'white')).getbbox()
        assert box, 'blank '+name
        d.text((8+450*col,y),f'{name} · {w} wide · center crop',font=font,fill='#192332')
        x,h=img.width//2,img.height//2
        sheet.paste(img.crop((x-210,h-19,x+210,h+19)),(8+450*col,y+28))
sheet.save(out/'ios.png')
android=stage/'android-evidence'
panel=Image.new('RGB',(1120,510),'#dbe2ea');draw=ImageDraw.Draw(panel)
for n,title in enumerate(['iOS SwiftUI reference','iOS model','Android Compose reference','Android model']):
    draw.text((8+n*280,12),title,font=font,fill='#192332')
for idx,(name,w) in enumerate((n,w) for n in ['Detail','Profile'] for w in [1440,390]):
    y=52+idx*110;draw.text((8,y),f'{name} · {w} wide · central 250 × 45 pixels',font=font,fill='#192332')
    for col,(source,mode) in enumerate([(native,'reference'),(native,'model'),(android,'reference'),(android,'model')]):
        suffix='View' if col<2 else 'Screen'
        img=Image.open(source/f'{name}{suffix}-{mode}-{w}.png').convert('RGB')
        x,h=img.width//2,img.height//2
        panel.paste(img.crop((x-125,h-22,x+125,h+23)),(8+280*col,y+32))
panel.save(out/'native-comparison.png')
with ZipFile(out/'native-device-captures.zip','w',ZIP_DEFLATED) as archive:
    for p in sorted(android.glob('*.png')):archive.write(p,'android/'+p.name)
    for p in sorted(stage.glob('*-device-*-*.png')):archive.write(p,'ios-edits/'+p.name)
    for p in sorted((stage/'native-ok').glob('*.png')):archive.write(p,'ios-reference/'+p.name)
