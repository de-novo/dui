from PIL import Image,ImageDraw
from pathlib import Path
import json,sys
base=Path(__file__).parent/'evidence';tmp=Path(sys.argv[1])
data=json.loads((base/'engines.json').read_text())
for w in [1440,390]:
 img=Image.new('RGB',(680,246),'white');d=ImageDraw.Draw(img)
 for j,k in enumerate(['Chrome / crop','Taffy / box','Yoga / box']):d.text((153+174*j,9),k,fill='#164739',font_size=17)
 for i,(case,vals) in enumerate(data['layout'].items()):
  top=46+i*62;d.text((12,top),case,fill='#162921',font_size=17)
  img.paste(Image.open(tmp/f'{w}-{case}.png'),(154,top))
  for j,k in enumerate(['Taffy','Yoga']):
   x,y,width,height=vals[k]['rect'];left=328+174*j;d.rectangle((left+x,top+y,left+x+width-1,top+y+height-1),fill='#196d5e')
 d.text((12,228),f'{w}px Chrome viewport | Top-left crop in CSS pixels',fill='#45574e',font_size=13)
 img.save(base/f'layout-{w}.png')
 img=Image.new('RGB',(765,130),'white');d=ImageDraw.Draw(img)
 for j,k in enumerate(['Chrome / crop','Original','simplecss','selectors']):d.text((10+190*j,9),k,fill='#164739',font_size=17)
 img.paste(Image.open(tmp/f'{w}-padding.png'),(10,42))
 for j,k in enumerate(['original','simplecss','selectors']):
  vals=data['style'][k]['padding'];d.text((200+j*190,55),('12px on 4 sides' if all(v.get('value')==12 for v in vals) else '4 unresolved'),fill='#162921',font_size=18)
 d.text((10,105),f'{w}px Chrome viewport | Same CSS and TSX input',fill='#45574e',font_size=15);img.save(base/f'style-{w}.png')
