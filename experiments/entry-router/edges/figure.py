from PIL import Image,ImageDraw
from pathlib import Path
import sys,json
base=Path(__file__).parent;root=Path(sys.argv[1]);data=json.loads((base/'evidence/results.json').read_text())
for w in [1440,390]:
 im=Image.new('RGB',(970,430),'white');d=ImageDraw.Draw(im)
 for i,(name,title) in enumerate([('todo','TodoMVC / filter click'),('bp','Bulletproof / Get started')]):
  photo=Image.open(root/f'{name}-{w}.png');photo.thumbnail((450,205));x=i*480+12
  im.paste(photo,(x,39));d.text((x,10),f'{w}px Chrome: {title}',fill='#143d35',font_size=18)
 for i,(m,row) in enumerate(data.items()):
  y=270+46*i;tp=sum(a[0] for a in row['scores_TP_FP_FN'].values())
  d.text((15,y),m,fill='#162b27',font_size=18)
  d.rectangle((115,y,115+tp*8,y+16),fill='#277557')
  d.text((510,y),f'{tp}/47   |   {row["medianMs"]:.1f} ms   |   {row["medianMiB"]:.1f} MiB   |   {len(row["negativeControlFalseEdges"])}/2 false controls',fill='#162b27',font_size=16)
 im.save(base/f'evidence/edges-{w}.png')
