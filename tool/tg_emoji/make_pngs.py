# Telegram emoji PNG'lari -> pngs/emoji_u<kodlar>.png (niqob qo'shiladi, 256 rang).
import re, struct, os, io, sys
from PIL import Image
import imagequant
from emoji_data import *
meta=open(SRC+'/assets/emoji/metadata.bin','rb').read()
masks={}
for k in range(len(meta)//4):
    e,m=struct.unpack_from('<HH',meta,k*4); masks[e]=m
OUT='pngs'; os.makedirs(OUT,exist_ok=True)
jobs=[]; seen={}
for p,sec in enumerate(data):
    for i,e in enumerate(sec):
        cps=[ord(c) for c in e if ord(c) not in (0xfe0f,0xfe0e)]
        name='emoji_u'+'_'.join('%04x'%c for c in cps)
        if name in seen: continue
        seen[name]=1; jobs.append((p,i,name))
def work(j):
    p,i,name=j
    dst=f'{OUT}/{name}.png'
    if os.path.exists(dst) and os.path.getsize(dst)>0: return
    im=Image.open(f'{SRC}/assets/emoji/{p}_{i}.png').convert('RGBA')
    key=p*4096+i
    if key in masks:
        im.putalpha(Image.open(f'{SRC}/assets/emoji/masks/{masks[key]}.png').convert('L'))
    q=imagequant.quantize_pil_image(im, dithering_level=1.0, max_colors=256, min_quality=0, max_quality=100)
    b=io.BytesIO(); q.save(b,'PNG',optimize=True)
    d=b.getvalue()
    open(dst+'.tmp','wb').write(d); os.replace(dst+'.tmp',dst)
if __name__=='__main__':
    from multiprocessing import Pool
    with Pool(4) as pool: pool.map(work, jobs, chunksize=8)
    print('files',len(jobs))
